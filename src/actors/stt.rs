/// Speech-to-Text actor (Whisper pipeline).
///
/// Waits for a new AudioChunk, encodes it as a 16-bit WAV in memory,
/// and sends it to the OpenAI Whisper API for transcription.
/// The resulting text is published to the TranscribedText slot.
use veecle_os::runtime::{InitializedReader, Writer};

use crate::types::{AudioChunk, TranscribedText};

#[veecle_os::runtime::actor]
pub async fn stt_actor(
    mut audio_in: InitializedReader<'_, AudioChunk>,
    mut text_out: Writer<'_, TranscribedText>,
) -> std::convert::Infallible {
    let client = reqwest::Client::new();
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

    loop {
        let chunk = audio_in.wait_for_update().await.read_cloned();
        veecle_os::telemetry::info!("STT: transcribing", seq = format!("{}", chunk.seq));

        let wav = encode_wav(&chunk.samples, chunk.sample_rate);

        let part = reqwest::multipart::Part::bytes(wav)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .unwrap();

        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", "whisper-1")
            .text("language", "en");

        match client
            .post("https://api.openai.com/v1/audio/transcriptions")
            .bearer_auth(&api_key)
            .multipart(form)
            .send()
            .await
        {
            Ok(resp) => match resp.json::<serde_json::Value>().await {
                Ok(json) => {
                    let text = json["text"].as_str().unwrap_or("").trim().to_string();
                    if !text.is_empty() {
                        veecle_os::telemetry::info!("STT: transcribed", text = format!("{text}"));
                        text_out
                            .write(TranscribedText { text, seq: chunk.seq })
                            .await;
                    }
                }
                Err(e) => {
                    veecle_os::telemetry::error!("STT: parse error", error = format!("{e}"))
                }
            },
            Err(e) => {
                veecle_os::telemetry::error!("STT: request error", error = format!("{e}"))
            }
        }
    }
}

/// Encode f32 PCM samples as a 16-bit signed WAV in memory.
pub fn encode_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
    for &s in samples {
        writer
            .write_sample((s * 32_767.0).clamp(-32_768.0, 32_767.0) as i16)
            .unwrap();
    }
    writer.finalize().unwrap();
    cursor.into_inner()
}
