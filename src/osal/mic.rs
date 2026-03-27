/// Microphone abstraction — OSAL extension for USB audio capture.
///
/// cpal::Stream is not guaranteed to be Send, so all audio I/O runs in a
/// dedicated std::thread. Complete speech segments are delivered to the
/// async actor via a tokio::sync::mpsc channel.
/// The pause flag (Arc<AtomicBool>) is set by the actor to mute capture
/// during TTS playback, preventing the robot from hearing itself.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::sync::mpsc;

// VAD parameters — mirror Python's audio_capture.py constants exactly.
const VAD_THRESHOLD: f32 = 0.0035;
const SILENCE_THRESHOLD_MS: u64 = 800;
const MIN_AUDIO_DURATION_SECS: f32 = 0.5;
const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Handle returned by MicAbstraction::start().
/// The actor uses `rx` to receive processed audio chunks and `paused` to
/// signal the capture thread to discard audio during TTS.
pub struct MicHandle {
    pub rx: mpsc::Receiver<Vec<f32>>,
    pub native_rate: u32,
    pub paused: Arc<AtomicBool>,
}

pub trait MicAbstraction {
    fn start(self) -> MicHandle;
}

// ── Real implementation (cpal) ────────────────────────────────────────────────

pub struct CpalMic {
    native_rate: u32,
    native_channels: u16,
}

impl CpalMic {
    pub fn new() -> Self {
        use cpal::traits::DeviceTrait;
        let host = cpal::default_host();

        // On Linux: prefer hw:/plughw: ALSA devices; fall back to default.
        // On other platforms (macOS, Windows): use the default input device directly.
        let (device, config) = find_input_device(&host);

        println!("[CpalMic] Using device: {}", device.name().unwrap_or_default());
        Self {
            native_rate: config.sample_rate().0,
            native_channels: config.channels(),
        }
    }
}

impl MicAbstraction for CpalMic {
    fn start(self) -> MicHandle {
        #[cfg(target_os = "macos")]
        request_microphone_permission_macos();

        let paused = Arc::new(AtomicBool::new(false));
        let paused_thread = paused.clone();
        let (tx, rx) = mpsc::channel::<Vec<f32>>(4);

        std::thread::Builder::new()
            .name("cpal-capture".into())
            .spawn(move || capture_thread(tx, paused_thread, self.native_rate, self.native_channels))
            .expect("Failed to spawn audio capture thread");

        MicHandle {
            rx,
            native_rate: self.native_rate,
            paused,
        }
    }
}

// AVFoundation FFI — macOS only.
// The extern blocks are at module level so #[link] is resolved at link time.
#[cfg(target_os = "macos")]
#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {
    static AVMediaTypeAudio: *const std::ffi::c_void;
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn objc_getClass(name: *const std::ffi::c_char) -> *const std::ffi::c_void;
    fn sel_registerName(name: *const std::ffi::c_char) -> *const std::ffi::c_void;
    #[link_name = "objc_msgSend"]
    fn av_capture_device_request_access(
        cls: *const std::ffi::c_void,
        sel: *const std::ffi::c_void,
        media_type: *const std::ffi::c_void,
        handler: *const std::ffi::c_void,
    );
}

/// Request macOS microphone permission via AVFoundation before opening the stream.
///
/// AVFoundation shows the dialog on the main thread's run loop. We must NOT block
/// the calling thread — instead we spawn a dedicated thread that waits for the
/// callback while the main/Tokio thread stays free to process UI events.
#[cfg(target_os = "macos")]
fn request_microphone_permission_macos() {
    use std::sync::{Arc, Condvar, Mutex};

    let result = Arc::new((Mutex::new(None::<bool>), Condvar::new()));
    let result2 = result.clone();

    // ObjC BOOL is i8 on macOS; 0 = false, non-zero = true.
    let block: block2::RcBlock<dyn Fn(i8)> = block2::RcBlock::new(move |granted: i8| {
        let (lock, cvar) = &*result2;
        *lock.lock().unwrap() = Some(granted != 0);
        cvar.notify_one();
    });

    unsafe {
        let cls = objc_getClass(b"AVCaptureDevice\0".as_ptr() as *const std::ffi::c_char);
        let sel = sel_registerName(
            b"requestAccessForMediaType:completionHandler:\0".as_ptr()
                as *const std::ffi::c_char,
        );
        let block_ptr = &*block as *const block2::Block<dyn Fn(i8)> as *const std::ffi::c_void;
        av_capture_device_request_access(cls, sel, AVMediaTypeAudio, block_ptr);
    }

    // Spawn a thread to wait for the callback so the main thread stays free
    // (AVFoundation dispatches the dialog UI on the main run loop).
    let result3 = result.clone();
    std::thread::spawn(move || {
        let (lock, cvar) = &*result3;
        let mut guard = lock.lock().unwrap();
        while guard.is_none() {
            guard = cvar.wait(guard).unwrap();
        }
        if guard.unwrap() {
            println!("[CpalMic] Microphone permission granted");
        } else {
            eprintln!("[CpalMic] Microphone permission denied — audio will be silent");
        }
    });

    // Keep block alive on the calling thread briefly so AVFoundation can copy it.
    // After Block_copy the ObjC runtime owns the block; we can drop our handle.
    std::thread::sleep(std::time::Duration::from_millis(50));
    drop(block);
}

/// Select the best available input device for the current platform.
fn find_input_device(host: &cpal::Host) -> (cpal::Device, cpal::SupportedStreamConfig) {
    use cpal::traits::{DeviceTrait, HostTrait};

    #[cfg(target_os = "linux")]
    {
        // Prefer hw: then plughw: (ALSA direct/plugin devices).
        host.input_devices()
            .expect("Failed to enumerate input devices")
            .filter(|d| {
                d.name()
                    .map(|n| n.starts_with("hw:") || n.starts_with("plughw:"))
                    .unwrap_or(false)
            })
            .find_map(|d| d.default_input_config().ok().map(|c| (d, c)))
            .or_else(|| {
                host.default_input_device()
                    .and_then(|d| d.default_input_config().ok().map(|c| (d, c)))
            })
            .expect("No usable audio input device found")
    }

    #[cfg(not(target_os = "linux"))]
    {
        host.default_input_device()
            .and_then(|d| d.default_input_config().ok().map(|c| (d, c)))
            .expect("No usable audio input device found")
    }
}

fn capture_thread(
    tx: mpsc::Sender<Vec<f32>>,
    paused: Arc<AtomicBool>,
    native_rate: u32,
    native_channels: u16,
) {
    use cpal::traits::{DeviceTrait, StreamTrait};

    let host = cpal::default_host();
    let (device, supported) = find_input_device(&host);
    let sample_format = supported.sample_format();

    // Use the device's native channel count; downmix to mono in the callbacks.
    let config = cpal::StreamConfig {
        channels: native_channels,
        sample_rate: cpal::SampleRate(native_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    // Bridge cpal callback (any thread) → std mpsc (VAD loop below).
    let (raw_tx, raw_rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(64);

    // Downmix interleaved multi-channel frames to mono by averaging.
    let downmix = move |interleaved: &[f32]| -> Vec<f32> {
        let ch = native_channels as usize;
        interleaved
            .chunks(ch)
            .map(|frame| frame.iter().sum::<f32>() / ch as f32)
            .collect()
    };

    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            let raw_tx = raw_tx.clone();
            device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let _ = raw_tx.try_send(downmix(data));
                },
                |e| eprintln!("[cpal] stream error: {e}"),
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let raw_tx = raw_tx.clone();
            device.build_input_stream(
                &config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let f: Vec<f32> = data.iter().map(|&s| s as f32 / 32_768.0).collect();
                    let _ = raw_tx.try_send(downmix(&f));
                },
                |e| eprintln!("[cpal] stream error: {e}"),
                None,
            )
        }
        _ => {
            let raw_tx = raw_tx.clone();
            device.build_input_stream(
                &config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    let f: Vec<f32> =
                        data.iter().map(|&s| s as f32 / 65_535.0 * 2.0 - 1.0).collect();
                    let _ = raw_tx.try_send(downmix(&f));
                },
                |e| eprintln!("[cpal] stream error: {e}"),
                None,
            )
        }
    }
    .expect("Failed to build input stream");

    drop(raw_tx); // only the closure's clones keep the channel open

    stream.play().expect("Failed to start audio stream");
    println!(
        "[CpalMic] Capturing at {native_rate} Hz, {native_channels} ch, format={sample_format:?}"
    );

    // VAD processing loop — runs entirely in this thread.
    let mut buffer: Vec<f32> = Vec::new();
    let mut last_voice: Option<std::time::Instant> = None;
    let mut frame_count = 0u64;
    let mut zero_frame_count = 0u64;

    loop {
        match raw_rx.recv_timeout(std::time::Duration::from_millis(50)) {
            Ok(samples) => {
                if paused.load(Ordering::Relaxed) {
                    buffer.clear();
                    last_voice = None;
                    continue;
                }

                let energy: f32 =
                    samples.iter().map(|s| s.abs()).sum::<f32>() / samples.len() as f32;
                frame_count += 1;

                // Warn if macOS is returning all-zero samples (mic permission not granted).
                if energy == 0.0 {
                    zero_frame_count += 1;
                    #[cfg(target_os = "macos")]
                    if zero_frame_count == 50 {
                        eprintln!(
                            "[CpalMic] WARNING: microphone is silent. \
                             If you just granted permission, restart the app. \
                             Check: System Settings > Privacy & Security > Microphone"
                        );
                    }
                } else {
                    zero_frame_count = 0;
                }

                if frame_count % 10 == 0 {
                    println!(
                        "[VAD] energy={energy:.6} threshold={VAD_THRESHOLD:.6}"
                    );
                }

                if energy > VAD_THRESHOLD {
                    if last_voice.is_none() {
                        println!("[VAD] Voice detected (energy={energy:.6})");
                    }
                    last_voice = Some(std::time::Instant::now());
                }

                if last_voice.is_some() {
                    buffer.extend_from_slice(&samples);

                    let silence_ms =
                        last_voice.map(|t| t.elapsed().as_millis() as u64).unwrap_or(0);

                    if silence_ms >= SILENCE_THRESHOLD_MS {
                        let raw: Vec<f32> = buffer.drain(..).collect();
                        last_voice = None;

                        let min_samples =
                            (MIN_AUDIO_DURATION_SECS * native_rate as f32) as usize;

                        if raw.len() >= min_samples {
                            let duration = raw.len() as f32 / native_rate as f32;
                            println!("[VAD] Captured {duration:.2}s of audio");

                            let processed = preprocess(raw, native_rate);
                            if tx.blocking_send(processed).is_err() {
                                eprintln!("[CpalMic] Actor channel closed — stopping");
                                break;
                            }
                        } else {
                            println!("[VAD] Audio too short, discarded");
                        }
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// Normalise to 90 % of peak amplitude, then resample to 16 kHz.
/// Mirrors Python's _normalize_audio + _resample_to_16k.
fn preprocess(samples: Vec<f32>, from_rate: u32) -> Vec<f32> {
    // Normalise
    let max = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    let normalised: Vec<f32> = if max > 0.0 {
        samples.iter().map(|s| (s / max * 0.9).clamp(-1.0, 1.0)).collect()
    } else {
        samples
    };

    // Resample (linear interpolation)
    if from_rate == TARGET_SAMPLE_RATE {
        return normalised;
    }
    let ratio = from_rate as f64 / TARGET_SAMPLE_RATE as f64;
    let out_len = (normalised.len() as f64 / ratio) as usize;
    (0..out_len)
        .map(|i| {
            let src = i as f64 * ratio;
            let idx = src as usize;
            let frac = (src - idx as f64) as f32;
            match normalised.get(idx + 1) {
                Some(&next) => normalised[idx] * (1.0 - frac) + next * frac,
                None => normalised.get(idx).copied().unwrap_or(0.0),
            }
        })
        .collect()
}

// ── Mock implementation (dev) ─────────────────────────────────────────────────

pub struct MockMic;

impl MockMic {
    pub fn new() -> Self {
        println!("[MockMic] Initialised — no audio will be captured");
        Self
    }
}

impl MicAbstraction for MockMic {
    fn start(self) -> MicHandle {
        let paused = Arc::new(AtomicBool::new(false));
        let (_tx, rx) = mpsc::channel::<Vec<f32>>(4);
        // The sender is intentionally dropped — the actor will wait forever,
        // which is the correct behaviour for a dev machine without a mic.
        MicHandle {
            rx,
            native_rate: TARGET_SAMPLE_RATE,
            paused,
        }
    }
}

// ── Platform type alias ───────────────────────────────────────────────────────

pub type Mic = CpalMic;
