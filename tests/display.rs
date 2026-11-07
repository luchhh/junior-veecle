use junior_protocol::{Emotion, State};
use junior_veecle::actors::display::display_state;
use junior_veecle::types::RobotCommand;

#[test]
fn display_state_priority() {
    // (sleeping, speaking, thinking, voice)
    assert_eq!(display_state(false, false, false, false), State::Idle);
    assert_eq!(display_state(false, false, false, true), State::Listening);
    assert_eq!(display_state(false, false, true, true), State::Thinking);
    assert_eq!(display_state(false, true, true, true), State::Speaking);
    assert_eq!(display_state(true, true, true, true), State::Sleeping);
}

#[test]
fn response_emotion_parses() {
    let cmds =
        RobotCommand::from_response(r#"{"say":"","emotion":"surprised","actions":[]}"#).unwrap();
    assert!(matches!(cmds.as_slice(), [RobotCommand::SetEmotion { emotion: Emotion::Surprised }]));

    assert!(RobotCommand::from_response(r#"{"say":"","emotion":"bored","actions":[]}"#).is_err());
}
