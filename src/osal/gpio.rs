/// L298N motor driver abstraction.
///
/// Pin mapping (from Python firmware/__init__.py, RPi 5 / gpiochip4):
///   Direction: GPIO 17, 22, 23, 24
///   PWM enable: GPIO 12 (motor A / right side), GPIO 13 (motor B / left side)
///
/// Motor direction truth table:
///   forward:        17=L 22=H 23=L 24=H  (A fwd,  B fwd)
///   reverse:        17=H 22=L 23=H 24=L  (A rev,  B rev)
///   left_forward:   17=L 22=H 23=L 24=L  (A fwd,  B coast) — pivot left going forward
///   right_forward:  17=L 22=L 23=L 24=H  (A coast,B fwd)   — pivot right going forward
///   left_backward:  17=H 22=L 23=L 24=L  (A rev,  B coast) — pivot left going backward
///   right_backward: 17=L 22=L 23=H 24=L  (A coast,B rev)   — pivot right going backward
///   stop:           PWM=0, all LOW
pub trait GpioAbstraction: Send + 'static {
    fn forward(&mut self, power: f64);
    fn reverse(&mut self, power: f64);
    fn left_forward(&mut self, power: f64);
    fn right_forward(&mut self, power: f64);
    fn left_backward(&mut self, power: f64);
    fn right_backward(&mut self, power: f64);
    fn stop(&mut self);
}

// ── Real implementation (Linux / Raspberry Pi) ────────────────────────────────

#[cfg(target_os = "linux")]
pub use real::RppalGpio;

#[cfg(target_os = "linux")]
mod real {
    use rppal::gpio::{Gpio, OutputPin};

    use super::GpioAbstraction;

    const PIN_17: u8 = 17;
    const PIN_22: u8 = 22;
    const PIN_23: u8 = 23;
    const PIN_24: u8 = 24;
    const PIN_PWM_A: u8 = 12;
    const PIN_PWM_B: u8 = 13;
    const PWM_FREQ: f64 = 1000.0;

    pub struct RppalGpio {
        pin17: OutputPin,
        pin22: OutputPin,
        pin23: OutputPin,
        pin24: OutputPin,
        pwm_a: OutputPin,
        pwm_b: OutputPin,
    }

    impl RppalGpio {
        pub fn new() -> Self {
            let gpio = Gpio::new().expect("Failed to initialise GPIO");
            Self {
                pin17: gpio.get(PIN_17).unwrap().into_output(),
                pin22: gpio.get(PIN_22).unwrap().into_output(),
                pin23: gpio.get(PIN_23).unwrap().into_output(),
                pin24: gpio.get(PIN_24).unwrap().into_output(),
                pwm_a: gpio.get(PIN_PWM_A).unwrap().into_output(),
                pwm_b: gpio.get(PIN_PWM_B).unwrap().into_output(),
            }
        }

        fn set_direction(&mut self, p17: bool, p22: bool, p23: bool, p24: bool) {
            if p17 { self.pin17.set_high() } else { self.pin17.set_low() }
            if p22 { self.pin22.set_high() } else { self.pin22.set_low() }
            if p23 { self.pin23.set_high() } else { self.pin23.set_low() }
            if p24 { self.pin24.set_high() } else { self.pin24.set_low() }
        }

        fn set_pwm(&mut self, duty: f64) {
            self.set_pwm_split(duty, duty);
        }

        /// Set Motor A and Motor B enable independently.
        /// Passing 0.0 disables the enable pin (true coast / high impedance).
        fn set_pwm_split(&mut self, duty_a: f64, duty_b: f64) {
            if duty_a > 0.0 {
                let _ = self.pwm_a.set_pwm_frequency(PWM_FREQ, duty_a.clamp(0.0, 100.0) / 100.0);
            } else {
                let _ = self.pwm_a.clear_pwm();
            }
            if duty_b > 0.0 {
                let _ = self.pwm_b.set_pwm_frequency(PWM_FREQ, duty_b.clamp(0.0, 100.0) / 100.0);
            } else {
                let _ = self.pwm_b.clear_pwm();
            }
        }
    }

    impl GpioAbstraction for RppalGpio {
        fn forward(&mut self, power: f64) {
            self.set_direction(false, true, false, true);
            self.set_pwm(power);
        }
        fn reverse(&mut self, power: f64) {
            self.set_direction(true, false, true, false);
            self.set_pwm(power);
        }
        fn left_forward(&mut self, power: f64) {
            // Motor A (right) drives, Motor B (left) truly coasts — ENB disabled.
            self.set_direction(false, true, false, false);
            self.set_pwm_split(power, 0.0);
        }
        fn right_forward(&mut self, power: f64) {
            // Motor B (left) drives, Motor A (right) truly coasts — ENA disabled.
            self.set_direction(false, false, false, true);
            self.set_pwm_split(0.0, power);
        }
        fn left_backward(&mut self, power: f64) {
            // Motor A (right) drives backward, Motor B (left) truly coasts.
            self.set_direction(true, false, false, false);
            self.set_pwm_split(power, 0.0);
        }
        fn right_backward(&mut self, power: f64) {
            // Motor B (left) drives backward, Motor A (right) truly coasts.
            self.set_direction(false, false, true, false);
            self.set_pwm_split(0.0, power);
        }
        fn stop(&mut self) {
            self.set_pwm(0.0);
            self.set_direction(false, false, false, false);
        }
    }

    impl Drop for RppalGpio {
        fn drop(&mut self) {
            self.stop();
        }
    }
}

// ── Mock implementation (macOS / dev) ─────────────────────────────────────────

pub struct MockGpio;

impl MockGpio {
    pub fn new() -> Self {
        println!("[MockGpio] Initialised");
        Self
    }
}

impl GpioAbstraction for MockGpio {
    fn forward(&mut self, power: f64)        { println!("[MockGpio] forward @ {power:.0}%") }
    fn reverse(&mut self, power: f64)        { println!("[MockGpio] reverse @ {power:.0}%") }
    fn left_forward(&mut self, power: f64)   { println!("[MockGpio] left_forward @ {power:.0}%") }
    fn right_forward(&mut self, power: f64)  { println!("[MockGpio] right_forward @ {power:.0}%") }
    fn left_backward(&mut self, power: f64)  { println!("[MockGpio] left_backward @ {power:.0}%") }
    fn right_backward(&mut self, power: f64) { println!("[MockGpio] right_backward @ {power:.0}%") }
    fn stop(&mut self)                       { println!("[MockGpio] stop") }
}

// ── Platform type alias ───────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
pub type Gpio = RppalGpio;

#[cfg(not(target_os = "linux"))]
pub type Gpio = MockGpio;
