//! 8x8 bitmaps: one byte per row, top row first, MSB = leftmost column.

use junior_protocol::{Emotion, State};

pub type Frame = [u8; 8];

const NEUTRAL: Frame = [
    0b0000_0000,
    0b0110_0110,
    0b0110_0110,
    0b0000_0000,
    0b0000_0000,
    0b0011_1100,
    0b0000_0000,
    0b0000_0000,
];

const BLINK: Frame = [
    0b0000_0000,
    0b0000_0000,
    0b0110_0110,
    0b0000_0000,
    0b0000_0000,
    0b0011_1100,
    0b0000_0000,
    0b0000_0000,
];

/// Wide-open eyes with a pulsing "o" mouth.
const LISTENING: [Frame; 2] = [
    [
        0b0000_0000,
        0b0110_0110,
        0b0110_0110,
        0b0110_0110,
        0b0000_0000,
        0b0001_1000,
        0b0001_1000,
        0b0000_0000,
    ],
    [
        0b0000_0000,
        0b0110_0110,
        0b0110_0110,
        0b0110_0110,
        0b0000_0000,
        0b0000_0000,
        0b0000_0000,
        0b0000_0000,
    ],
];

/// Loading dots filling up from left to right.
const THINKING: [Frame; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0b1100_0000, 0b1100_0000, 0, 0, 0],
    [0, 0, 0, 0b1101_1000, 0b1101_1000, 0, 0, 0],
    [0, 0, 0, 0b1101_1011, 0b1101_1011, 0, 0, 0],
];

/// Mouth closing and opening.
const SPEAKING: [Frame; 2] = [
    NEUTRAL,
    [
        0b0000_0000,
        0b0110_0110,
        0b0110_0110,
        0b0000_0000,
        0b0011_1100,
        0b0010_0100,
        0b0011_1100,
        0b0000_0000,
    ],
];

const HAPPY: Frame = [
    0b0000_0000,
    0b0110_0110,
    0b0110_0110,
    0b0000_0000,
    0b1000_0001,
    0b0100_0010,
    0b0011_1100,
    0b0000_0000,
];

const SAD: Frame = [
    0b0000_0000,
    0b0110_0110,
    0b0110_0110,
    0b0000_0000,
    0b0000_0000,
    0b0011_1100,
    0b0100_0010,
    0b1000_0001,
];

const SURPRISED: Frame = [
    0b0000_0000,
    0b0110_0110,
    0b0110_0110,
    0b0000_0000,
    0b0001_1000,
    0b0010_0100,
    0b0010_0100,
    0b0001_1000,
];

/// Eyebrows slanting down towards the middle.
const ANGRY: Frame = [
    0b1100_0011,
    0b0010_0100,
    0b0110_0110,
    0b0110_0110,
    0b0000_0000,
    0b0011_1100,
    0b0100_0010,
    0b0000_0000,
];

const LOVE: Frame = [
    0b0000_0000,
    0b0110_0110,
    0b1111_1111,
    0b1111_1111,
    0b0111_1110,
    0b0011_1100,
    0b0001_1000,
    0b0000_0000,
];

/// X eyes: no message from the Pi for a while.
pub const DISCONNECTED: Frame = [
    0b0000_0000,
    0b1010_0101,
    0b0100_0010,
    0b1010_0101,
    0b0000_0000,
    0b0011_1100,
    0b0000_0000,
    0b0000_0000,
];

/// The frame to show for `state` at `millis` since boot.
pub fn state(state: State, millis: u64) -> Frame {
    match state {
        // Blink for 150 ms every 3 s.
        State::Idle if millis % 3000 < 150 => BLINK,
        State::Idle => NEUTRAL,
        State::Listening => LISTENING[(millis / 400 % 2) as usize],
        State::Thinking => THINKING[(millis / 250 % 4) as usize],
        State::Speaking => SPEAKING[(millis / 150 % 2) as usize],
    }
}

pub fn emotion(emotion: Emotion) -> Frame {
    match emotion {
        Emotion::Happy => HAPPY,
        Emotion::Sad => SAD,
        Emotion::Surprised => SURPRISED,
        Emotion::Angry => ANGRY,
        Emotion::Love => LOVE,
    }
}
