/// Combined STT + LLM actor (Audio pipeline).
///
/// Reads a raw AudioChunk, asks the AI model to interpret it directly via
/// tool calls, and publishes the command sequence to the Store.
/// Thinking is set for as long as the request is in flight.
use veecle_os::runtime::{InitializedReader, Writer};

use crate::llm_client::{AudioPrompt, MemoryClient};
use crate::types::{AudioChunk, CommandSequence, RobotCommand, Thinking};

#[veecle_os::runtime::actor]
pub async fn audio_llm_actor<C: AudioPrompt + MemoryClient + 'static>(
    #[init_context] client: C,
    mut audio_in: InitializedReader<'_, AudioChunk>,
    mut commands_out: Writer<'_, CommandSequence>,
    mut thinking_out: Writer<'_, Thinking>,
) -> std::convert::Infallible {
    let mut session_history: Vec<String> = Vec::new();
    let mut last_speak: Option<String> = None;

    loop {
        let memories = match &last_speak {
            Some(query) => match MemoryClient::search(&client, query).await {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("[AudioLLM] mem0 search error: {e}");
                    vec![]
                }
            },
            None => vec![],
        };

        if memories.is_empty() {
            eprintln!("[AudioLLM] memories: (none)");
        } else {
            eprintln!("[AudioLLM] memories ({}):", memories.len());
            for m in &memories {
                eprintln!("  - {m}");
            }
        }

        let chunk = audio_in.wait_for_update().await.read_cloned();
        println!("[AudioLLM] processing chunk seq={} samples={}", chunk.seq, chunk.samples.len());
        veecle_os::telemetry::info!(
            "AudioLLM: processing chunk",
            seq = format!("{}", chunk.seq)
        );

        thinking_out.write(Thinking { active: true }).await;
        let result =
            AudioPrompt::ask(&client, &chunk.samples, chunk.sample_rate, &session_history, &memories)
                .await;
        thinking_out.write(Thinking { active: false }).await;

        match result {
            Ok(commands) => {
                veecle_os::telemetry::info!(
                    "AudioLLM: commands received",
                    count = format!("{}", commands.len())
                );

                let speak_text: Option<String> = commands.iter().find_map(|c| {
                    if let RobotCommand::Speak { body } = c { Some(body.clone()) } else { None }
                });

                if let Some(text) = &speak_text {
                    eprintln!("[AudioLLM] adding memory: {text:?}");
                    if let Err(e) = MemoryClient::add(&client, text).await {
                        eprintln!("[AudioLLM] mem0 add error: {e}");
                    }
                    session_history.push(text.clone());
                    last_speak = Some(text.clone());
                }

                commands_out
                    .write(CommandSequence { commands, seq: chunk.seq })
                    .await;
            }
            Err(e) => {
                eprintln!("[AudioLLM] error: {e}");
                veecle_os::telemetry::error!("AudioLLM: request error", error = format!("{e}"))
            }
        }
    }
}
