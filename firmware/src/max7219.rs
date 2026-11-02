//! Minimal driver for a single MAX7219 driving one 8x8 LED matrix.

use embassy_stm32::gpio::Output;
use embassy_stm32::mode::Blocking;
use embassy_stm32::spi::{self, Spi};

use crate::faces::Frame;

const REG_DIGIT0: u8 = 0x01;
const REG_DECODE_MODE: u8 = 0x09;
const REG_INTENSITY: u8 = 0x0A;
const REG_SCAN_LIMIT: u8 = 0x0B;
const REG_SHUTDOWN: u8 = 0x0C;
const REG_DISPLAY_TEST: u8 = 0x0F;

pub struct Max7219<'d> {
    spi: Spi<'d, Blocking, spi::mode::Master>,
    cs: Output<'d>,
}

impl<'d> Max7219<'d> {
    pub fn new(spi: Spi<'d, Blocking, spi::mode::Master>, cs: Output<'d>) -> Self {
        Self { spi, cs }
    }

    /// Safe to call repeatedly: it doesn't touch the digit registers, so the frame stays visible.
    pub fn init(&mut self) {
        self.write(REG_DISPLAY_TEST, 0);
        self.write(REG_DECODE_MODE, 0);
        self.write(REG_SCAN_LIMIT, 7);
        self.write(REG_INTENSITY, 0); // 0..=15, 0 is the dimmest
        self.write(REG_SHUTDOWN, 1);
    }

    pub fn draw(&mut self, frame: &Frame) {
        for (i, row) in rotate_cw(frame).iter().enumerate() {
            self.write(REG_DIGIT0 + i as u8, *row);
        }
    }

    fn write(&mut self, reg: u8, data: u8) {
        self.cs.set_low();
        self.spi.blocking_write(&[reg, data]).unwrap();
        self.cs.set_high();
    }
}

/// Our module shows frames turned 270° clockwise, so turn them 90° clockwise before sending.
fn rotate_cw(frame: &Frame) -> Frame {
    let mut rotated = [0u8; 8];
    for (row, out) in rotated.iter_mut().enumerate() {
        for col in 0..8 {
            if frame[7 - col] & (0x80 >> row) != 0 {
                *out |= 0x80 >> col;
            }
        }
    }
    rotated
}
