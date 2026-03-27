use junior_veecle::actors::llm::LlmActor;
use junior_veecle::llm_client::{ClientError, TextPrompt};
use junior_veecle::types::{CommandSequence, RobotCommand, TranscribedText};
use veecle_os::runtime::{Reader, Writer};

struct MockTextPrompt {
    response: &'static str,
}

impl TextPrompt for MockTextPrompt {
    fn ask(&self, _text: &str) -> impl std::future::Future<Output = Result<String, ClientError>> + Send {
        let response = self.response;
        async move { Ok(response.to_string()) }
    }
}

#[test]
fn llm_actor_parses_commands_from_mock_response() {
    let mock = MockTextPrompt { response: r#"[{"command": "forward", "ms": 1000}]"# };

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
            assert!(matches!(seq.commands[0], RobotCommand::Forward { ms: 1000 }));
        },
    });
}
