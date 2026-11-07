use std::time::{Duration, Instant};

use junior_veecle::actors::audio_llm::{MAX_SESSION_TURNS, SESSION_WINDOW, SessionHistory};
use junior_veecle::llm_client::Turn;

fn turn(n: usize) -> Turn {
    Turn { user: Some(format!("user {n}")), junior: Some(format!("junior {n}")) }
}

#[test]
fn session_keeps_only_the_last_hour() {
    let start = Instant::now();
    let mut session = SessionHistory::default();
    session.push(start, turn(1));
    session.push(start + Duration::from_secs(30 * 60), turn(2));

    assert_eq!(session.recent(start + Duration::from_secs(59 * 60)), vec![turn(1), turn(2)]);
    assert_eq!(session.recent(start + SESSION_WINDOW), vec![turn(2)]);
    assert_eq!(session.recent(start + SESSION_WINDOW * 2), vec![]);
}

#[test]
fn session_is_capped() {
    let now = Instant::now();
    let mut session = SessionHistory::default();
    for n in 0..MAX_SESSION_TURNS + 5 {
        session.push(now, turn(n));
    }

    let recent = session.recent(now);
    assert_eq!(recent.len(), MAX_SESSION_TURNS);
    assert_eq!(recent[0], turn(5));
}
