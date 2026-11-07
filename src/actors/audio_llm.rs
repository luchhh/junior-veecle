/// Combined STT + LLM actor (Audio pipeline).
///
/// Reads a raw AudioChunk, asks the AI model to interpret it directly via
/// tool calls, and publishes the command sequence to the Store.
/// Thinking is set for as long as the request is in flight.
///
/// The model also returns a `heard` transcript of what the user said. It is kept
/// out of the command sequence and, together with Junior's reply, goes into the
/// session history and long-term memory.
use veecle_os::runtime::{InitializedReader, Writer};

use crate::llm_client::{AudioPrompt, MemoryClient, Turn};
use crate::types::{AudioChunk, CommandSequence, RobotCommand, Thinking};

/// Long-term memories put in the prompt each turn, newest first.
const MAX_MEMORIES: usize = 100;
/// Turns of the current conversation sent along with each request.
const MAX_HISTORY_TURNS: usize = 20;

#[veecle_os::runtime::actor]
pub async fn audio_llm_actor<C: AudioPrompt + MemoryClient + 'static>(
    #[init_context] client: C,
    mut audio_in: InitializedReader<'_, AudioChunk>,
    mut commands_out: Writer<'_, CommandSequence>,
    mut thinking_out: Writer<'_, Thinking>,
) -> std::convert::Infallible {
    let mut session_history: Vec<Turn> = Vec::new();

    loop {
        // Fetched while waiting for the user, so it costs no reply time and
        // includes whatever the previous turn stored.
        let memories = match MemoryClient::recent(&client, MAX_MEMORIES).await {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[AudioLLM] mem0 recent error: {e}");
                vec![]
            }
        };
        eprintln!("[AudioLLM] memories: {}", memories.len());

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

                let (turn, commands) = split_turn(commands);
                if turn.user.is_none() {
                    eprintln!("[AudioLLM] warning: no heard transcript in the response");
                }

                commands_out
                    .write(CommandSequence { commands, seq: chunk.seq })
                    .await;

                // Junior is already acting; storing the turn doesn't hold up the reply.
                if turn.user.is_some() || turn.junior.is_some() {
                    eprintln!("[AudioLLM] remembering: {turn:?}");
                    if let Err(e) = MemoryClient::add(&client, &turn).await {
                        eprintln!("[AudioLLM] mem0 add error: {e}");
                    }
                    session_history.push(turn);
                    if session_history.len() > MAX_HISTORY_TURNS {
                        session_history.remove(0);
                    }
                }
            }
            Err(e) => {
                eprintln!("[AudioLLM] error: {e}");
                veecle_os::telemetry::error!("AudioLLM: request error", error = format!("{e}"))
            }
        }
    }
}

/// Separates the conversation turn (what was heard and said) from the commands for the robot.
fn split_turn(commands: Vec<RobotCommand>) -> (Turn, Vec<RobotCommand>) {
    let mut turn = Turn::default();
    let mut actions = Vec::with_capacity(commands.len());
    for command in commands {
        match command {
            RobotCommand::Heard { text } => turn.user = Some(text),
            RobotCommand::Speak { ref body } => {
                turn.junior = Some(match turn.junior.take() {
                    Some(said) => format!("{said} {body}"),
                    None => body.clone(),
                });
                actions.push(command);
            }
            other => actions.push(other),
        }
    }
    (turn, actions)
}
