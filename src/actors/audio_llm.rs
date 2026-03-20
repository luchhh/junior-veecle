/// Combined STT + LLM actor (Audio pipeline).
///
/// Reads a raw AudioChunk, base64-encodes it as WAV, and sends it directly
/// to GPT-4o Audio Preview which handles both transcription and reasoning
/// in a single API call. Publishes the parsed command sequence to the Store.
///
/// This replaces the stt_actor + llm_actor pair used in the Whisper pipeline.
use base64::Engine as _;
use veecle_os::runtime::{InitializedReader, Writer};

use crate::actors::stt::encode_wav;
use crate::types::{AudioChunk, CommandSequence, RobotCommand};

const SYSTEM_PROMPT: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/prompts/system.md"));

#[veecle_os::runtime::actor]
pub async fn audio_llm_actor(
    mut audio_in: InitializedReader<'_, AudioChunk>,
    mut commands_out: Writer<'_, CommandSequence>,
) -> std::convert::Infallible {
    let client = reqwest::Client::new();
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

    loop {
        let chunk = audio_in.wait_for_update().await.read_cloned();
        veecle_os::telemetry::info!(
            "AudioLLM: processing chunk",
            seq = format!("{}", chunk.seq)
        );

        let wav_bytes = encode_wav(&chunk.samples, chunk.sample_rate);
        let audio_b64 = base64::engine::general_purpose::STANDARD.encode(&wav_bytes);

        let body = serde_json::json!({
            "model": "gpt-4o-audio-preview",
            "messages": [
                { "role": "system", "content": SYSTEM_PROMPT },
                {
                    "role": "user",
                    "content": [{
                        "type": "input_audio",
                        "input_audio": { "data": audio_b64, "format": "wav" },
                    }],
                },
            ],
            "temperature": 0.7,
        });

        match client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&api_key)
            .json(&body)
            .send()
            .await
        {
            Ok(resp) => match resp.json::<serde_json::Value>().await {
                Ok(json) => {
                    let content = json["choices"][0]["message"]["content"]
                        .as_str()
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    match serde_json::from_str::<Vec<RobotCommand>>(&content) {
                        Ok(commands) => {
                            veecle_os::telemetry::info!(
                                "AudioLLM: commands parsed",
                                count = format!("{}", commands.len())
                            );
                            commands_out
                                .write(CommandSequence { commands, seq: chunk.seq })
                                .await;
                        }
                        Err(e) => veecle_os::telemetry::error!(
                            "AudioLLM: JSON parse error",
                            error = format!("{e}"),
                            content = format!("{content}")
                        ),
                    }
                }
                Err(e) => veecle_os::telemetry::error!(
                    "AudioLLM: response parse error",
                    error = format!("{e}")
                ),
            },
            Err(e) => veecle_os::telemetry::error!(
                "AudioLLM: request error",
                error = format!("{e}")
            ),
        }
    }
}
