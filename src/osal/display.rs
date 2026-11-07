/// LED face abstraction — the STM32 display board on the Pi's UART.
///
/// Messages are `junior_protocol` lines written to DISPLAY_SERIAL (default
/// /dev/serial0, i.e. GPIO14 TX / pin 8) at 115200 baud. The robot keeps
/// running without a face if the port can't be opened, retrying every 5 s.
use junior_protocol::Msg;

pub trait DisplayAbstraction: Send + 'static {
    fn send(&mut self, msg: Msg);
}

// ── Real implementation (Linux / Raspberry Pi) ────────────────────────────────

#[cfg(target_os = "linux")]
pub use real::SerialDisplay;

#[cfg(target_os = "linux")]
mod real {
    use std::io::Write as _;
    use std::time::{Duration, Instant};

    use junior_protocol::Msg;
    use serialport::SerialPort;

    use super::DisplayAbstraction;

    const BAUD_RATE: u32 = 115_200;
    const RETRY_INTERVAL: Duration = Duration::from_secs(5);

    pub struct SerialDisplay {
        path: String,
        port: Option<Box<dyn SerialPort>>,
        last_attempt: Option<Instant>,
    }

    impl SerialDisplay {
        pub fn new() -> Self {
            let path = std::env::var("DISPLAY_SERIAL").unwrap_or_else(|_| "/dev/serial0".into());
            let mut display = Self { path, port: None, last_attempt: None };
            display.connect();
            display
        }

        fn connect(&mut self) {
            self.last_attempt = Some(Instant::now());
            match serialport::new(&self.path, BAUD_RATE)
                .timeout(Duration::from_millis(100))
                .open()
            {
                Ok(port) => {
                    println!("[SerialDisplay] Connected on {}", self.path);
                    self.port = Some(port);
                }
                Err(e) => eprintln!("[SerialDisplay] Can't open {}: {e}", self.path),
            }
        }
    }

    impl DisplayAbstraction for SerialDisplay {
        fn send(&mut self, msg: Msg) {
            if self.port.is_none()
                && self.last_attempt.is_none_or(|t| t.elapsed() >= RETRY_INTERVAL)
            {
                self.connect();
            }
            let Some(port) = self.port.as_mut() else { return };
            if let Err(e) = writeln!(port, "{msg}") {
                eprintln!("[SerialDisplay] Write failed: {e}");
                self.port = None;
            }
        }
    }
}

// ── Mock implementation (macOS / dev) ─────────────────────────────────────────

pub struct MockDisplay {
    last: Option<Msg>,
}

impl MockDisplay {
    pub fn new() -> Self {
        println!("[MockDisplay] Initialised");
        Self { last: None }
    }
}

impl DisplayAbstraction for MockDisplay {
    fn send(&mut self, msg: Msg) {
        // The actor re-sends the state as a keepalive; only print changes.
        if self.last != Some(msg) {
            println!("[MockDisplay] {msg}");
            self.last = Some(msg);
        }
    }
}

// ── Platform type alias ───────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
pub type Display = SerialDisplay;

#[cfg(not(target_os = "linux"))]
pub type Display = MockDisplay;
