/// Microphone capture actor.
///
/// Owns a MicHandle (spawned in the OSAL mic layer) and relays processed
/// speech segments to the AudioChunk Store slot.
/// Syncs the pause flag from CaptureState on every iteration so the capture
/// thread discards audio while the robot is speaking.
use std::sync::atomic::Ordering;

use veecle_os::runtime::{Reader, Writer};

use crate::osal::mic::{Mic, MicAbstraction as _};
use crate::types::{AudioChunk, CaptureState};

#[veecle_os::runtime::actor]
pub async fn audio_capture_actor(
    mut audio_out: Writer<'_, AudioChunk>,
    capture_state: Reader<'_, CaptureState>,
) -> std::convert::Infallible {
    let handle = Mic::new().start();
    let mut rx = handle.rx;
    let paused_flag = handle.paused;

    let mut seq = 0u64;

    loop {
        // Sync pause state from the Store into the capture thread's atomic flag.
        let is_paused =
            capture_state.read(|s: Option<&CaptureState>| s.map(|s| s.paused).unwrap_or(false));
        paused_flag.store(is_paused, Ordering::Relaxed);

        // Wait up to 100 ms for an audio chunk, then loop to refresh pause state.
        match tokio::time::timeout(
            tokio::time::Duration::from_millis(100),
            rx.recv(),
        )
        .await
        {
            Ok(Some(samples)) => {
                seq += 1;
                veecle_os::telemetry::info!(
                    "Audio chunk ready",
                    seq = format!("{seq}"),
                    samples = format!("{}", samples.len())
                );
                audio_out
                    .write(AudioChunk {
                        samples,
                        sample_rate: 16_000,
                        seq,
                    })
                    .await;
            }
            Ok(None) => {
                veecle_os::telemetry::error!("Mic channel closed unexpectedly");
            }
            Err(_timeout) => {
                // No audio yet — loop to refresh pause flag.
            }
        }
    }
}
