/// Combined STT + LLM actor (Audio pipeline).
///
/// Reads a raw AudioChunk, asks the AI model to interpret it directly,
/// parses the response into a command sequence, and publishes it to the Store.
use veecle_os::runtime::{InitializedReader, Writer};

use crate::llm_client::AudioPrompt;
use crate::types::{AudioChunk, CommandSequence, RobotCommand};

#[veecle_os::runtime::actor]
pub async fn audio_llm_actor<C: AudioPrompt + 'static>(
    #[init_context] client: C,
    mut audio_in: InitializedReader<'_, AudioChunk>,
    mut commands_out: Writer<'_, CommandSequence>,
) -> std::convert::Infallible {
    loop {
        let chunk = audio_in.wait_for_update().await.read_cloned();
        veecle_os::telemetry::info!(
            "AudioLLM: processing chunk",
            seq = format!("{}", chunk.seq)
        );

        match client.ask(&chunk.samples, chunk.sample_rate).await {
            Ok(raw) => match RobotCommand::parse_many(&raw) {
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
                    "AudioLLM: parse error",
                    error = format!("{e}"),
                    content = format!("{raw}")
                ),
            },
            Err(e) => veecle_os::telemetry::error!("AudioLLM: request error", error = format!("{e}")),
        }
    }
}
