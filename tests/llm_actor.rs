use junior_veecle::actors::llm::LlmActor;
use junior_veecle::llm_client::{ClientError, TextPrompt};
use junior_veecle::types::{CommandSequence, RobotCommand, TranscribedText};
use veecle_os::runtime::{Reader, Writer};

struct MockTextPrompt {
    commands: Vec<RobotCommand>,
}

impl TextPrompt for MockTextPrompt {
    fn ask(
        &self,
        _text: &str,
    ) -> impl std::future::Future<Output = Result<Vec<RobotCommand>, ClientError>> + Send {
        let commands = self.commands.clone();
        async move { Ok(commands) }
    }
}

#[test]
fn llm_actor_forwards_commands_from_client() {
    let mock = MockTextPrompt {
        commands: vec![RobotCommand::Forward { secs: 2.0 }],
    };

    veecle_os_test::block_on_future(veecle_os_test::execute! {
        store: [TranscribedText, CommandSequence],

        actors: [
            LlmActor<MockTextPrompt>: mock,
        ],

        validation: async |
            mut text_in: Writer<'a, TranscribedText>,
            mut commands_out: Reader<'a, CommandSequence>,
        | {
            text_in.write(TranscribedText { text: "go forward one meter".into(), seq: 1 }).await;

            let seq = commands_out.wait_for_update().await.read_cloned().unwrap();
            assert_eq!(seq.commands.len(), 1);
            assert!(matches!(seq.commands[0], RobotCommand::Forward { secs } if secs == 2.0));
        },
    });
}
