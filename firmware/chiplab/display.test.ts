// Chiplab test suite for the junior display firmware.
//
// Plays the Pi: sends protocol lines on usart2 and checks the simulated MAX7219 matrix.
// Board: { model: "stm32f4_discovery", uarts: ["usart2"],
//          devices: [{ model: "max7219", name: "matrix", bus: "spi2", chip_select: "PB12" }] }
import { expect } from "jsr:@std/expect";

// Chiplab workarounds, both for simulator behaviour that doesn't happen on the real board:
// - The firmware clock runs at 0.625x virtual time (timers at 10 MHz instead of the 16 MHz the
//   firmware configures), so firmware durations take 1.6x as long here. Set to 1 once fixed.
const CLOCK_SCALE = 1.6;
// - A write landing at the exact instant a reply line completes loses its second byte, so the
//   test waits a little after every reply before writing again.
const SETTLE_MS = 10;

// Mirrors firmware/src/faces.rs, rows top to bottom, leftmost column first.
const NEUTRAL = ["00000000", "01100110", "01100110", "00000000", "00000000", "00111100", "00000000", "00000000"];
const BLINK = ["00000000", "00000000", "01100110", "00000000", "00000000", "00111100", "00000000", "00000000"];
const LISTENING = [
  ["00000000", "01100110", "01100110", "01100110", "00000000", "00011000", "00011000", "00000000"],
  ["00000000", "01100110", "01100110", "01100110", "00000000", "00000000", "00000000", "00000000"],
];
const THINKING = [
  ["00000000", "00000000", "00000000", "00000000", "00000000", "00000000", "00000000", "00000000"],
  ["00000000", "00000000", "00000000", "11000000", "11000000", "00000000", "00000000", "00000000"],
  ["00000000", "00000000", "00000000", "11011000", "11011000", "00000000", "00000000", "00000000"],
  ["00000000", "00000000", "00000000", "11011011", "11011011", "00000000", "00000000", "00000000"],
];
const SPEAKING = [
  NEUTRAL,
  ["00000000", "01100110", "01100110", "00000000", "00111100", "00100100", "00111100", "00000000"],
];
const SLEEPING = [
  ["00000000", "00000000", "00000000", "00000000", "00000000", "11100000", "01000000", "11100000"],
  ["00001111", "00000010", "00000100", "00001111", "00000000", "11100000", "01000000", "11100000"],
  ["00001111", "00000010", "00000100", "00001111", "00000000", "00000000", "00000000", "00000000"],
  ["00000000", "00000000", "00000000", "00000000", "00000000", "00000000", "00000000", "00000000"],
];
const EMOTIONS: Record<string, string[]> = {
  HAPPY: ["00000000", "01100110", "01100110", "00000000", "10000001", "01000010", "00111100", "00000000"],
  SAD: ["00000000", "01100110", "01100110", "00000000", "00000000", "00111100", "01000010", "10000001"],
  SURPRISED: ["00000000", "01100110", "01100110", "00000000", "00011000", "00100100", "00100100", "00011000"],
  ANGRY: ["11000011", "00100100", "01100110", "01100110", "00000000", "00111100", "01000010", "00000000"],
  LOVE: ["00000000", "01100110", "11111111", "11111111", "01111110", "00111100", "00011000", "00000000"],
};
const DISCONNECTED = ["00000000", "10100101", "01000010", "10100101", "00000000", "00111100", "00000000", "00000000"];

const key = (frame: string[]) => frame.join("/");

/** The face as drawn in faces.rs: undoes the 90° clockwise turn the driver applies for our module. */
async function frame(): Promise<string[]> {
  const matrix = await simulation.getLedMatrix("matrix");
  const pixels = await matrix.pixels();
  return pixels.map((_, row) => pixels.map((_, col) => (pixels[col][7 - row] ? "1" : "0")).join(""));
}

async function send(line: string) {
  await fireEvent.write(await simulation.getUart("usart2"), line + "\n");
}

/** Sends `line`, waits for the firmware's answer (e.g. `OK STATE IDLE`) and for the next frame. */
async function command(line: string, reply = "OK") {
  await send(line);
  const outcome = await simulation.runUntilUartLine(`${reply} ${line}`, 500);
  expect(outcome.kind).toBe("matched");
  // The display loop picks messages up on its next 50 ms (firmware) tick.
  await simulation.runForMillis(100);
}

/** Runs for `millis` with a heartbeat every 500 ms, returning every distinct frame seen (sampled every 50 ms). */
async function runAlive(millis: number): Promise<Set<string>> {
  const seen = new Set<string>();
  for (let t = 0; t < millis; t += 50) {
    if (t % 500 === 0) await send("HB");
    await simulation.runForMillis(50);
    seen.add(key(await frame()));
  }
  return seen;
}

Deno.test("junior display", async (t) => {
  await t.step("boots, says READY and shows the disconnected face", async () => {
    expect((await simulation.runUntilUartLine("READY", 2000)).kind).toBe("matched");
    await simulation.runForMillis(SETTLE_MS);
    expect(await frame()).toEqual(DISCONNECTED);
  });

  await t.step("a heartbeat connects and shows the idle face", async () => {
    await command("HB");
    await simulation.runForMillis(100);
    expect([key(NEUTRAL), key(BLINK)]).toContain(key(await frame()));
  });

  await t.step("THINKING cycles through the loading dots", async () => {
    await command("STATE THINKING");
    const seen = await runAlive(1500);
    expect([...seen].sort()).toEqual(THINKING.map(key).sort());
  });

  await t.step("SPEAKING opens and closes the mouth", async () => {
    await command("STATE SPEAKING");
    const seen = await runAlive(1000);
    expect([...seen].sort()).toEqual(SPEAKING.map(key).sort());
  });

  await t.step("LISTENING pulses with wide eyes", async () => {
    await command("STATE LISTENING");
    const seen = await runAlive(1500);
    expect([...seen].sort()).toEqual(LISTENING.map(key).sort());
  });

  await t.step("SLEEPING floats Z's up", async () => {
    await command("STATE SLEEPING");
    // A full cycle is 4 frames of 600 ms (firmware time), plus margin for the start phase.
    const seen = await runAlive(3000 * CLOCK_SCALE);
    expect([...seen].sort()).toEqual(SLEEPING.map(key).sort());
    // Back to LISTENING for the emotion step, which expects it afterwards.
    await command("STATE LISTENING");
  });

  await t.step("an emotion shows for 3 s, then the state animation returns", async () => {
    await command("EMOTION HAPPY");
    const during = await runAlive(2700 * CLOCK_SCALE);
    expect([...during]).toEqual([key(EMOTIONS.HAPPY)]);
    await runAlive(500 * CLOCK_SCALE);
    const after = await runAlive(1000);
    expect([...after].sort()).toEqual(LISTENING.map(key).sort());
  });

  await t.step("every emotion has its face", async () => {
    for (const [name, expected] of Object.entries(EMOTIONS)) {
      await command(`EMOTION ${name}`);
      await simulation.runForMillis(100);
      expect(await frame()).toEqual(expected);
    }
  });

  await t.step("unknown and overlong lines are rejected", async () => {
    await command("STATE DANCING", "ERR");
    await command("hb", "ERR");
    await send("EMOTION " + "X".repeat(40));
    expect((await simulation.runUntilUartLine("ERR EMOTION XXX", 500)).kind).toBe("matched");
    await simulation.runForMillis(SETTLE_MS);
  });

  await t.step("silence for over 2 s disconnects and resets the state", async () => {
    await command("STATE THINKING");
    await simulation.runForMillis(2500 * CLOCK_SCALE);
    expect(await frame()).toEqual(DISCONNECTED);

    await command("HB");
    await simulation.runForMillis(100);
    expect([key(NEUTRAL), key(BLINK)]).toContain(key(await frame()));
  });

  await t.step("the MAX7219 setup has no hardware warnings", async () => {
    const matrix = await simulation.getLedMatrix("matrix");
    expect(await matrix.warnings()).toEqual([]);
  });
});
