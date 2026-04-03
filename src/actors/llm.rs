/// LLM actor (Whisper pipeline).
///
/// Reads transcribed text, asks the AI model to generate commands via tool
/// calls, and publishes the command sequence to the Store.
use veecle_os::runtime::{InitializedReader, Writer};

use crate::llm_client::TextPrompt;
use crate::types::{CommandSequence, TranscribedText};

#[veecle_os::runtime::actor]
pub async fn llm_actor<C: TextPrompt + 'static>(
    #[init_context] client: C,
    mut text_in: InitializedReader<'_, TranscribedText>,
    mut commands_out: Writer<'_, CommandSequence>,
) -> std::convert::Infallible {
    loop {
        let input = text_in.wait_for_update().await.read_cloned();
        veecle_os::telemetry::info!("LLM: sending text", text = format!("{}", input.text));

        match client.ask(&input.text).await {
            Ok(commands) => {
                veecle_os::telemetry::info!(
                    "LLM: commands received",
                    count = format!("{}", commands.len())
                );
                commands_out
                    .write(CommandSequence { commands, seq: input.seq })
                    .await;
            }
            Err(e) => veecle_os::telemetry::error!("LLM: request error", error = format!("{e}")),
        }
    }
}
