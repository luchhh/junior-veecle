/// Combined STT + LLM actor (Audio pipeline).
///
/// Reads a raw AudioChunk, asks the AI model to interpret it directly via
/// tool calls, and publishes the command sequence to the Store.
/// Thinking is set for as long as the request is in flight.
///
/// The same audio is transcribed in parallel. The transcript and Junior's reply go
/// into two levels of memory: the session (the last hour, word for word) and
/// long-term memory (mem0, which keeps only lasting facts).
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use veecle_os::runtime::{InitializedReader, Writer};

use crate::llm_client::{AudioPrompt, AudioToText, MemoryClient, Turn};
use crate::types::{AudioChunk, CommandSequence, RobotCommand, Thinking};

/// Long-term memories put in the prompt each turn, newest first.
const MAX_MEMORIES: usize = 100;
/// How far back the session goes.
pub const SESSION_WINDOW: Duration = Duration::from_secs(60 * 60);
/// Cap on session turns, so a long chat doesn't make every request huge.
pub const MAX_SESSION_TURNS: usize = 30;

/// Session memory: the conversation word for word, for the last [SESSION_WINDOW].
/// Lives only in this process, so a restart starts a new session.
#[derive(Debug, Default)]
pub struct SessionHistory {
    turns: VecDeque<(Instant, Turn)>,
}

impl SessionHistory {
    pub fn push(&mut self, at: Instant, turn: Turn) {
        self.turns.push_back((at, turn));
        if self.turns.len() > MAX_SESSION_TURNS {
            self.turns.pop_front();
        }
    }

    /// The turns from the last [SESSION_WINDOW] before `now`, oldest first. Older ones are dropped.
    pub fn recent(&mut self, now: Instant) -> Vec<Turn> {
        while let Some((at, _)) = self.turns.front()
            && now.duration_since(*at) >= SESSION_WINDOW
        {
            self.turns.pop_front();
        }
        self.turns.iter().map(|(_, turn)| turn.clone()).collect()
    }
}

#[veecle_os::runtime::actor]
pub async fn audio_llm_actor<C: AudioPrompt + AudioToText + MemoryClient + 'static>(
    #[init_context] client: C,
    mut audio_in: InitializedReader<'_, AudioChunk>,
    mut commands_out: Writer<'_, CommandSequence>,
    mut thinking_out: Writer<'_, Thinking>,
) -> std::convert::Infallible {
    let mut session = SessionHistory::default();

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
        let history = session.recent(Instant::now());

        // Transcription runs alongside the request but is only awaited after the commands
        // are out, so it never delays the reply (it is usually, but not always, faster).
        let transcription = AudioToText::transcribe(&client, &chunk.samples, chunk.sample_rate);
        tokio::pin!(transcription);
        let mut transcript = None;

        thinking_out.write(Thinking { active: true }).await;
        let result = {
            let ask = AudioPrompt::ask(
                &client,
                &chunk.samples,
                chunk.sample_rate,
                &history,
                &memories,
            );
            tokio::pin!(ask);
            loop {
                tokio::select! {
                    result = &mut ask => break result,
                    text = &mut transcription, if transcript.is_none() => transcript = Some(text),
                }
            }
        };
        thinking_out.write(Thinking { active: false }).await;

        match result {
            Ok(commands) => {
                veecle_os::telemetry::info!(
                    "AudioLLM: commands received",
                    count = format!("{}", commands.len())
                );

                let junior = spoken_text(&commands);

                commands_out
                    .write(CommandSequence { commands, seq: chunk.seq })
                    .await;

                // Junior is already acting; finishing the transcript and storing the turn
                // doesn't hold up the reply.
                let transcript = match transcript {
                    Some(text) => text,
                    None => transcription.await,
                };
                let user = match transcript {
                    Ok(text) if !text.is_empty() => Some(text),
                    Ok(_) => None,
                    Err(e) => {
                        eprintln!("[AudioLLM] transcription error: {e}");
                        None
                    }
                };
                let turn = Turn { user, junior };

                if turn.user.is_some() || turn.junior.is_some() {
                    eprintln!("[AudioLLM] remembering: {turn:?}");
                    if let Err(e) = MemoryClient::add(&client, &turn).await {
                        eprintln!("[AudioLLM] mem0 add error: {e}");
                    }
                    session.push(Instant::now(), turn);
                }
            }
            Err(e) => {
                eprintln!("[AudioLLM] error: {e}");
                veecle_os::telemetry::error!("AudioLLM: request error", error = format!("{e}"))
            }
        }
    }
}

/// Everything Junior says in this turn, joined into one line.
fn spoken_text(commands: &[RobotCommand]) -> Option<String> {
    let said: Vec<&str> = commands
        .iter()
        .filter_map(|c| match c {
            RobotCommand::Speak { body } => Some(body.as_str()),
            _ => None,
        })
        .collect();
    (!said.is_empty()).then(|| said.join(" "))
}
