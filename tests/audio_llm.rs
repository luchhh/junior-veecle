use std::sync::{Arc, Mutex};

use junior_veecle::actors::audio_llm::AudioLlmActor;
use junior_protocol::Emotion;
use junior_veecle::llm_client::{
    AudioPrompt, AudioToText, ClientError, MemoryClient, Turn, WakeListener,
};
use junior_veecle::types::{AudioChunk, CommandSequence, RobotCommand, Sleeping, Thinking};
use veecle_os::runtime::{Reader, Writer};

/// What the actor handed to the client, so the test can check it afterwards.
#[derive(Default)]
struct Calls {
    /// (history, memories) for each `ask`.
    asks: Vec<(Vec<Turn>, Vec<String>)>,
    added: Vec<Turn>,
    /// Chunk numbers sent to the cloud model, and to the local wake listener.
    asked_chunks: Vec<usize>,
    local_chunks: Vec<usize>,
}

/// A chunk's length stands for its number n: the transcript is "user n" and Junior answers
/// "junior n", also going to sleep if n is `sleep_on`. Locally, chunk `WAKE_CHUNK` is the
/// wake-up phrase. Every added turn is remembered as a "fact".
struct MockClient {
    calls: Arc<Mutex<Calls>>,
    sleep_on: Option<usize>,
}

const WAKE_CHUNK: u64 = 11;

impl AudioPrompt for MockClient {
    fn ask(
        &self,
        samples: &[f32],
        _sample_rate: u32,
        history: &[Turn],
        memories: &[String],
    ) -> impl std::future::Future<Output = Result<Vec<RobotCommand>, ClientError>> + Send {
        let mut calls = self.calls.lock().unwrap();
        calls.asks.push((history.to_vec(), memories.to_vec()));
        calls.asked_chunks.push(samples.len());
        let n = samples.len();
        let mut commands = vec![RobotCommand::Speak { body: format!("junior {n}") }];
        if self.sleep_on == Some(n) {
            commands.push(RobotCommand::Sleep);
        }
        async move { Ok(commands) }
    }
}

impl AudioToText for MockClient {
    fn transcribe(
        &self,
        samples: &[f32],
        _sample_rate: u32,
    ) -> impl std::future::Future<Output = Result<String, ClientError>> + Send {
        let n = samples.len();
        async move { Ok(format!("user {n}")) }
    }
}

impl WakeListener for MockClient {
    fn transcribe_locally(
        &self,
        samples: &[f32],
        _sample_rate: u32,
    ) -> impl std::future::Future<Output = Result<String, ClientError>> + Send {
        self.calls.lock().unwrap().local_chunks.push(samples.len());
        let text = if samples.len() as u64 == WAKE_CHUNK { "Júnior, despierta." } else { "algo privado" };
        async move { Ok(text.to_string()) }
    }
}

impl MemoryClient for MockClient {
    fn recent(
        &self,
        _limit: usize,
    ) -> impl std::future::Future<Output = Result<Vec<String>, ClientError>> + Send {
        let facts = self.calls.lock().unwrap().added.iter().filter_map(|t| t.user.clone()).collect();
        async move { Ok(facts) }
    }

    fn add(
        &self,
        turn: &Turn,
    ) -> impl std::future::Future<Output = Result<(), ClientError>> + Send {
        self.calls.lock().unwrap().added.push(turn.clone());
        async { Ok(()) }
    }
}

fn chunk(seq: u64) -> AudioChunk {
    AudioChunk { samples: vec![0.0; seq as usize], sample_rate: 16_000, seq }
}

fn turn(n: u32) -> Turn {
    Turn { user: Some(format!("user {n}")), junior: Some(format!("junior {n}")) }
}

#[test]
fn audio_llm_remembers_the_transcript_and_the_reply() {
    let calls = Arc::new(Mutex::new(Calls::default()));
    let mock = MockClient { calls: calls.clone(), sleep_on: None };

    veecle_os_test::block_on_future(veecle_os_test::execute! {
        store: [AudioChunk, CommandSequence, Thinking, Sleeping],

        actors: [
            AudioLlmActor<MockClient>: mock,
        ],

        validation: async |
            mut audio_out: Writer<'a, AudioChunk>,
            mut commands_in: Reader<'a, CommandSequence>,
            mut _thinking: Reader<'a, Thinking>,
            mut _sleeping: Reader<'a, Sleeping>,
        | {
            // A third turn makes sure the first two were stored: the actor stores a turn before reading the next chunk.
            for seq in 1..=3 {
                audio_out.write(chunk(seq)).await;
                let sequence = commands_in.wait_for_update().await.read_cloned().unwrap();
                assert_eq!(sequence.seq, seq);
                assert!(
                    matches!(sequence.commands.as_slice(), [RobotCommand::Speak { body }] if *body == format!("junior {seq}")),
                    "unexpected commands: {:?}",
                    sequence.commands
                );
            }
        },
    });

    let calls = calls.lock().unwrap();
    assert_eq!(calls.added[..2], [turn(1), turn(2)]);
    // The second request carries the first turn, both as conversation history and as a memory.
    assert_eq!(calls.asks[0], (vec![], vec![]));
    assert_eq!(calls.asks[1], (vec![turn(1)], vec!["user 1".to_string()]));
    assert_eq!(calls.asks[2].0, vec![turn(1), turn(2)]);
}

#[test]
fn asleep_only_listens_locally_until_the_wake_up_phrase() {
    let calls = Arc::new(Mutex::new(Calls::default()));
    let mock = MockClient { calls: calls.clone(), sleep_on: Some(1) };

    veecle_os_test::block_on_future(veecle_os_test::execute! {
        store: [AudioChunk, CommandSequence, Thinking, Sleeping],

        actors: [
            AudioLlmActor<MockClient>: mock,
        ],

        validation: async |
            mut audio_out: Writer<'a, AudioChunk>,
            mut commands_in: Reader<'a, CommandSequence>,
            mut _thinking: Reader<'a, Thinking>,
            mut sleeping_in: Reader<'a, Sleeping>,
        | {
            // "Go to sleep": this turn still runs, then Junior sleeps.
            audio_out.write(chunk(1)).await;
            let sequence = commands_in.wait_for_update().await.read_cloned().unwrap();
            assert!(matches!(sequence.commands.as_slice(), [RobotCommand::Speak { .. }, RobotCommand::Sleep]));
            assert_eq!(sleeping_in.wait_for_update().await.read_cloned(), Some(Sleeping { active: true }));

            // The wake-up phrase is heard locally: a happy face, then awake again.
            audio_out.write(chunk(WAKE_CHUNK)).await;
            let sequence = commands_in.wait_for_update().await.read_cloned().unwrap();
            assert!(
                matches!(sequence.commands.as_slice(), [RobotCommand::SetEmotion { emotion: Emotion::Happy }]),
                "{:?}",
                sequence.commands
            );
            assert_eq!(sleeping_in.wait_for_update().await.read_cloned(), Some(Sleeping { active: false }));

            audio_out.write(chunk(2)).await;
            let sequence = commands_in.wait_for_update().await.read_cloned().unwrap();
            assert!(matches!(sequence.commands.as_slice(), [RobotCommand::Speak { body }] if body == "junior 2"));
        },
    });

    let calls = calls.lock().unwrap();
    // The chunk heard while asleep never reached the cloud model or the memory.
    assert_eq!(calls.asked_chunks, [1, 2]);
    assert_eq!(calls.local_chunks, [WAKE_CHUNK as usize]);
    assert!(calls.added.iter().all(|t| t.user.as_deref() != Some("user 11")), "{:?}", calls.added);
}
