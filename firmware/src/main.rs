//! Junior display: renders what the Pi is doing on a MAX7219 8x8 LED matrix.
//!
//! Wiring (STM32F4DISCOVERY):
//!   MAX7219 DIN → PB15 (SPI2_MOSI), CLK → PB13 (SPI2_SCK), CS → PB12
//!   Pi TX → PA3 (USART2_RX), Pi RX ← PA2 (USART2_TX), 115200 8N1
//!
//! The Pi speaks the line protocol defined in `junior_protocol`.
#![no_std]
#![no_main]

mod faces;
mod max7219;

use defmt::{info, warn};
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::peripherals::{PA2, PA3, USART2};
use embassy_stm32::spi::{self, Spi};
use embassy_stm32::usart::{self, BufferedUart};
use embassy_stm32::{Peri, bind_interrupts};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Instant, Ticker};
use embedded_io_async::{Read, Write};
use junior_protocol::{Msg, State};
use {defmt_rtt as _, panic_probe as _};

use crate::max7219::Max7219;

/// Without any message from the Pi for this long, show the disconnected face.
const LINK_TIMEOUT: Duration = Duration::from_secs(2);
/// How long an emotion stays on screen before the state animation comes back.
const EMOTION_DURATION: Duration = Duration::from_secs(3);
/// Re-send the MAX7219 setup this often, so a re-plugged module comes back on its own.
const REINIT_INTERVAL: Duration = Duration::from_secs(1);
const FRAME_INTERVAL: Duration = Duration::from_millis(50);
/// Longest accepted line, without the `\n`.
const MAX_LINE: usize = 32;

bind_interrupts!(struct Irqs {
    USART2 => usart::BufferedInterruptHandler<USART2>;
});

static MESSAGES: Channel<CriticalSectionRawMutex, Msg, 8> = Channel::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    spawner.spawn(uart_task(p.USART2, p.PA3, p.PA2).unwrap());

    let spi = Spi::new_blocking_txonly(p.SPI2, p.PB13, p.PB15, spi::Config::default());
    let cs = Output::new(p.PB12, Level::High, Speed::Low);
    let mut display = Max7219::new(spi, cs);
    // Green LED: lit while the Pi link is alive.
    let mut link_led = Output::new(p.PD12, Level::Low, Speed::Low);

    let mut state = State::Idle;
    let mut emotion = None;
    let mut last_message: Option<Instant> = None;
    let mut last_init: Option<Instant> = None;
    let mut shown = None;

    let mut ticker = Ticker::every(FRAME_INTERVAL);
    loop {
        while let Ok(msg) = MESSAGES.try_receive() {
            last_message = Some(Instant::now());
            match msg {
                Msg::Heartbeat => {}
                Msg::State(new) => state = new,
                Msg::Emotion(new) => emotion = Some((new, Instant::now() + EMOTION_DURATION)),
            }
        }

        let now = Instant::now();
        let connected = last_message.is_some_and(|t| now - t < LINK_TIMEOUT);
        if !connected {
            // Start from a clean slate when the Pi comes back.
            state = State::Idle;
            emotion = None;
        }
        link_led.set_level(Level::from(connected));

        let frame = match emotion {
            _ if !connected => faces::DISCONNECTED,
            Some((emotion, until)) if now < until => faces::emotion(emotion),
            _ => faces::state(state, now.as_millis()),
        };

        if last_init.is_none_or(|t| now - t >= REINIT_INTERVAL) {
            display.init();
            last_init = Some(now);
            shown = None;
        }
        if shown != Some(frame) {
            display.draw(&frame);
            shown = Some(frame);
        }

        ticker.next().await;
    }
}

/// Reads lines from the Pi, forwards parsed messages to the display loop and answers `OK`/`ERR`.
#[embassy_executor::task]
async fn uart_task(usart: Peri<'static, USART2>, rx: Peri<'static, PA3>, tx: Peri<'static, PA2>) {
    let mut tx_buf = [0u8; 128];
    let mut rx_buf = [0u8; 128];
    let mut uart = BufferedUart::new(
        usart,
        rx,
        tx,
        &mut tx_buf,
        &mut rx_buf,
        Irqs,
        usart::Config::default(), // 115200 8N1
    )
    .unwrap();

    let _ = uart.write_all(b"READY\r\n").await;
    info!("uart ready");

    let mut line = [0u8; MAX_LINE];
    let mut len = 0;
    let mut overflow = false;
    let mut chunk = [0u8; 32];
    loop {
        let n = match uart.read(&mut chunk).await {
            Ok(n) => n,
            Err(e) => {
                warn!("uart read error: {}", e);
                continue;
            }
        };
        for &byte in &chunk[..n] {
            if byte != b'\n' {
                if len < MAX_LINE {
                    line[len] = byte;
                    len += 1;
                } else {
                    overflow = true;
                }
                continue;
            }

            let text = core::str::from_utf8(&line[..len]).unwrap_or("").trim();
            let msg = if overflow { None } else { Msg::parse(text) };
            let reply: &[u8] = if msg.is_some() { b"OK " } else { b"ERR " };
            let _ = uart.write_all(reply).await;
            let _ = uart.write_all(text.as_bytes()).await;
            let _ = uart.write_all(b"\r\n").await;
            if let Some(msg) = msg {
                MESSAGES.send(msg).await;
            }
            len = 0;
            overflow = false;
        }
    }
}
