use junior_protocol::{Emotion, State};
use junior_veecle::actors::display::display_state;
use junior_veecle::types::RobotCommand;

#[test]
fn display_state_priority() {
    // (speaking, thinking, voice)
    assert_eq!(display_state(false, false, false), State::Idle);
    assert_eq!(display_state(false, false, true), State::Listening);
    assert_eq!(display_state(false, true, true), State::Thinking);
    assert_eq!(display_state(true, true, true), State::Speaking);
}

#[test]
fn set_emotion_tool_call_parses() {
    let cmd = RobotCommand::from_tool_call("set_emotion", r#"{"emotion":"surprised"}"#).unwrap();
    assert!(matches!(cmd, RobotCommand::SetEmotion { emotion: Emotion::Surprised }));

    assert!(RobotCommand::from_tool_call("set_emotion", r#"{"emotion":"bored"}"#).is_err());
}
