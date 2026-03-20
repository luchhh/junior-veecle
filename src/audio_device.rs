/// Auto-detect the ALSA card number of the first USB audio device via `aplay -l`.
/// Mirrors Python's audio_device.py logic exactly.
pub fn get_audio_device() -> u32 {
    detect_usb_audio_device().unwrap_or_else(|| {
        println!("No USB audio device found, falling back to card 1");
        1
    })
}

fn detect_usb_audio_device() -> Option<u32> {
    let output = std::process::Command::new("aplay")
        .arg("-l")
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        // Format: "card 1: UACDemoV10 [UACDemoV1.0], device 0: USB Audio [USB Audio]"
        if line.starts_with("card") && line.to_uppercase().contains("USB") {
            if let Some(num) = line
                .strip_prefix("card ")
                .and_then(|s| s.split(':').next())
                .and_then(|s| s.trim().parse().ok())
            {
                println!("Auto-detected USB audio card: {num}");
                return Some(num);
            }
        }
    }

    None
}
