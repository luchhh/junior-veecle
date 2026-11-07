use std::time::{Duration, Instant};

use junior_veecle::actors::command_executor::{MAX_QUIET, Quiet};
use junior_veecle::types::RobotCommand;

const MINUTE: Duration = Duration::from_secs(60);

#[test]
fn quiet_lasts_the_requested_minutes() {
    let now = Instant::now();
    let mut quiet = Quiet::default();
    assert!(!quiet.is_quiet(now));

    assert_eq!(quiet.start(5.0, now), 5 * MINUTE);
    assert!(quiet.is_quiet(now + 4 * MINUTE));
    assert!(!quiet.is_quiet(now + 5 * MINUTE));
}

#[test]
fn quiet_is_capped_at_an_hour() {
    let now = Instant::now();
    let mut quiet = Quiet::default();

    for minutes in [61.0, 1_000_000.0, f64::INFINITY] {
        assert_eq!(quiet.start(minutes, now), MAX_QUIET);
    }
    assert!(quiet.is_quiet(now + MAX_QUIET - MINUTE));
    assert!(!quiet.is_quiet(now + MAX_QUIET));
}

#[test]
fn zero_or_invalid_minutes_end_the_quiet() {
    let now = Instant::now();
    let mut quiet = Quiet::default();

    for minutes in [0.0, -3.0, f64::NAN] {
        quiet.start(10.0, now);
        assert_eq!(quiet.start(minutes, now), Duration::ZERO);
        assert!(!quiet.is_quiet(now));
    }
}

#[test]
fn be_quiet_action_parses() {
    let args = r#"{"say":"","emotion":"none","actions":[{"type":"be_quiet","value":15}]}"#;
    let cmds = RobotCommand::from_response(args).unwrap();
    assert!(matches!(cmds.as_slice(), [RobotCommand::BeQuiet { minutes }] if *minutes == 15.0));
}

#[test]
fn be_quiet_comes_before_speech() {
    // "You can talk again": the reply must not be muted by the quiet it ends.
    let args = r#"{"say":"¡Gracias!","emotion":"none","actions":[
        {"type":"forward","value":1},
        {"type":"be_quiet","value":0}
    ]}"#;
    let cmds = RobotCommand::from_response(args).unwrap();
    assert!(
        matches!(
            cmds.as_slice(),
            [RobotCommand::BeQuiet { .. }, RobotCommand::Speak { .. }, RobotCommand::Forward { .. }]
        ),
        "{cmds:?}"
    );
}
