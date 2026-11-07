use std::sync::{Arc, Mutex};

use junior_veecle::actors::audio_llm::AudioLlmActor;
use junior_veecle::llm_client::{AudioPrompt, AudioToText, ClientError, MemoryClient, Turn};
use junior_veecle::types::{AudioChunk, CommandSequence, RobotCommand, Thinking};
use veecle_os::runtime::{Reader, Writer};

/// What the actor handed to the client, so the test can check it afterwards.
#[derive(Default)]
struct Calls {
    /// (history, memories) for each `ask`.
    asks: Vec<(Vec<Turn>, Vec<String>)>,
    added: Vec<Turn>,
}

/// A chunk's length stands for its number n: the transcript is "user n" and Junior answers
/// "junior n". Every added turn is remembered as a "fact".
struct MockClient {
    calls: Arc<Mutex<Calls>>,
}

impl AudioPrompt for MockClient {
    fn ask(
        &self,
        samples: &[f32],
        _sample_rate: u32,
        history: &[Turn],
        memories: &[String],
    ) -> impl std::future::Future<Output = Result<Vec<RobotCommand>, ClientError>> + Send {
        self.calls.lock().unwrap().asks.push((history.to_vec(), memories.to_vec()));
        let n = samples.len();
        async move { Ok(vec![RobotCommand::Speak { body: format!("junior {n}") }]) }
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
    let mock = MockClient { calls: calls.clone() };

    veecle_os_test::block_on_future(veecle_os_test::execute! {
        store: [AudioChunk, CommandSequence, Thinking],

        actors: [
            AudioLlmActor<MockClient>: mock,
        ],

        validation: async |
            mut audio_out: Writer<'a, AudioChunk>,
            mut commands_in: Reader<'a, CommandSequence>,
            mut _thinking: Reader<'a, Thinking>,
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
