/// LLM actor (Whisper pipeline).
///
/// Reads transcribed text, sends it to GPT-4o-mini via the OpenAI chat
/// completions API using the robot system prompt, and publishes the parsed
/// JSON command sequence to the Store.
use veecle_os::runtime::{InitializedReader, Writer};

use crate::types::{CommandSequence, RobotCommand, TranscribedText};

const SYSTEM_PROMPT: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/prompts/system.md"));

#[veecle_os::runtime::actor]
pub async fn llm_actor(
    mut text_in: InitializedReader<'_, TranscribedText>,
    mut commands_out: Writer<'_, CommandSequence>,
) -> std::convert::Infallible {
    let client = reqwest::Client::new();
    let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

    loop {
        let input = text_in.wait_for_update().await.read_cloned();
        veecle_os::telemetry::info!("LLM: sending text", text = format!("{}", input.text));

        let body = serde_json::json!({
            "model": "gpt-4o-mini",
            "messages": [
                { "role": "system", "content": SYSTEM_PROMPT },
                { "role": "user",   "content": input.text },
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
                                "LLM: commands parsed",
                                count = format!("{}", commands.len())
                            );
                            commands_out
                                .write(CommandSequence { commands, seq: input.seq })
                                .await;
                        }
                        Err(e) => veecle_os::telemetry::error!(
                            "LLM: JSON parse error",
                            error = format!("{e}"),
                            content = format!("{content}")
                        ),
                    }
                }
                Err(e) => veecle_os::telemetry::error!(
                    "LLM: response parse error",
                    error = format!("{e}")
                ),
            },
            Err(e) => veecle_os::telemetry::error!(
                "LLM: request error",
                error = format!("{e}")
            ),
        }
    }
}
