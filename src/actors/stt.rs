/// Speech-to-Text actor (Whisper pipeline).
///
/// Waits for a new AudioChunk, transcribes it, and publishes the
/// resulting text to the TranscribedText slot.
use veecle_os::runtime::{InitializedReader, Writer};

use crate::llm_client::AudioToText;
use crate::types::{AudioChunk, TranscribedText};

#[veecle_os::runtime::actor]
pub async fn stt_actor<C: AudioToText + 'static>(
    #[init_context] client: C,
    mut audio_in: InitializedReader<'_, AudioChunk>,
    mut text_out: Writer<'_, TranscribedText>,
) -> std::convert::Infallible {
    loop {
        let chunk = audio_in.wait_for_update().await.read_cloned();
        veecle_os::telemetry::info!("STT: transcribing", seq = format!("{}", chunk.seq));

        match client.transcribe(&chunk.samples, chunk.sample_rate).await {
            Ok(text) if !text.is_empty() => {
                veecle_os::telemetry::info!("STT: transcribed", text = format!("{text}"));
                text_out
                    .write(TranscribedText { text, seq: chunk.seq })
                    .await;
            }
            Ok(_) => {}
            Err(e) => veecle_os::telemetry::error!("STT: error", error = format!("{e}")),
        }
    }
}
