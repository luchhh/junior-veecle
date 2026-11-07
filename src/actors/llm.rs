/// LLM actor (Whisper pipeline).
///
/// Reads transcribed text, asks the AI model to generate commands via tool
/// calls, and publishes the command sequence to the Store.
/// Thinking is set for as long as the request is in flight.
/// This pipeline has no sleep mode: it reports Junior as awake once, for the LED face.
use veecle_os::runtime::{InitializedReader, Writer};

use crate::llm_client::TextPrompt;
use crate::types::{CommandSequence, Sleeping, Thinking, TranscribedText};

#[veecle_os::runtime::actor]
pub async fn llm_actor<C: TextPrompt + 'static>(
    #[init_context] client: C,
    mut text_in: InitializedReader<'_, TranscribedText>,
    mut commands_out: Writer<'_, CommandSequence>,
    mut thinking_out: Writer<'_, Thinking>,
    mut sleeping_out: Writer<'_, Sleeping>,
) -> std::convert::Infallible {
    sleeping_out.write(Sleeping { active: false }).await;

    loop {
        let input = text_in.wait_for_update().await.read_cloned();
        veecle_os::telemetry::info!("LLM: sending text", text = format!("{}", input.text));

        thinking_out.write(Thinking { active: true }).await;
        let result = client.ask(&input.text).await;
        thinking_out.write(Thinking { active: false }).await;

        match result {
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
