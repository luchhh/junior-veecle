use junior_protocol::Emotion;
use junior_veecle::types::RobotCommand;

#[test]
fn response_becomes_emotion_then_speech_then_actions() {
    let cmds = RobotCommand::from_response(
        r#"{"say":"Ahí voy","emotion":"happy","actions":[
            {"type":"forward","value":1.5},
            {"type":"left","value":90},
            {"type":"happy_dance","value":0}
        ]}"#,
    )
    .unwrap();

    assert!(
        matches!(
            cmds.as_slice(),
            [
                RobotCommand::SetEmotion { emotion: Emotion::Happy },
                RobotCommand::Speak { body },
                RobotCommand::Forward { secs },
                RobotCommand::Left { deg },
                RobotCommand::HappyDance,
            ] if body == "Ahí voy" && *secs == 1.5 && *deg == 90.0
        ),
        "{cmds:?}"
    );
}

#[test]
fn empty_parts_produce_no_commands() {
    let cmds = RobotCommand::from_response(r#"{"say":"  ","emotion":"none","actions":[]}"#).unwrap();
    assert!(cmds.is_empty(), "{cmds:?}");
}

#[test]
fn unknown_action_is_an_error() {
    let args = r#"{"say":"","emotion":"none","actions":[{"type":"fly","value":1}]}"#;
    assert!(RobotCommand::from_response(args).is_err());
}
