//! Line protocol from the Pi to the junior display (STM32).
//!
//! One ASCII message per line, `\n`-terminated (surrounding whitespace, e.g. `\r`, is ignored):
//!
//! ```text
//! HB                 heartbeat; without any message for 2 s the display shows "disconnected"
//! STATE <state>      IDLE | LISTENING | THINKING | SPEAKING
//! EMOTION <emotion>  HAPPY | SAD | SURPRISED | ANGRY | LOVE (shown for a few seconds)
//! ```
//!
//! The display prints `READY` at boot and answers every line with `OK <line>` or `ERR <line>`.
#![no_std]

use core::fmt;

macro_rules! keyword_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $text:literal),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            $($variant),*
        }

        impl $name {
            pub const ALL: &[$name] = &[$($name::$variant),*];

            pub fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $text),*
                }
            }

            pub fn parse(text: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|v| v.as_str() == text)
            }
        }
    };
}

keyword_enum! {
    /// What the robot is doing right now.
    State {
        Idle => "IDLE",
        Listening => "LISTENING",
        Thinking => "THINKING",
        Speaking => "SPEAKING",
    }
}

keyword_enum! {
    /// A short-lived expression shown on top of the current state.
    Emotion {
        Happy => "HAPPY",
        Sad => "SAD",
        Surprised => "SURPRISED",
        Angry => "ANGRY",
        Love => "LOVE",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Msg {
    Heartbeat,
    State(State),
    Emotion(Emotion),
}

impl Msg {
    /// Parse one line, without its terminating `\n`.
    pub fn parse(line: &str) -> Option<Self> {
        let line = line.trim();
        match line.split_once(' ') {
            None if line == "HB" => Some(Msg::Heartbeat),
            Some(("STATE", state)) => State::parse(state).map(Msg::State),
            Some(("EMOTION", emotion)) => Emotion::parse(emotion).map(Msg::Emotion),
            _ => None,
        }
    }
}

/// Formats the message as a line, without the terminating `\n`.
impl fmt::Display for Msg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Msg::Heartbeat => f.write_str("HB"),
            Msg::State(state) => write!(f, "STATE {}", state.as_str()),
            Msg::Emotion(emotion) => write!(f, "EMOTION {}", emotion.as_str()),
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::string::ToString;

    #[test]
    fn parses_every_message() {
        assert_eq!(Msg::parse("HB"), Some(Msg::Heartbeat));
        assert_eq!(Msg::parse("STATE THINKING\r"), Some(Msg::State(State::Thinking)));
        assert_eq!(Msg::parse("EMOTION LOVE"), Some(Msg::Emotion(Emotion::Love)));
    }

    #[test]
    fn rejects_unknown_lines() {
        for line in ["", "hb", "STATE", "STATE DANCING", "EMOTION HAPPY NOW", "HB 1"] {
            assert_eq!(Msg::parse(line), None, "{line:?}");
        }
    }

    #[test]
    fn display_round_trips() {
        let all = State::ALL.iter().map(|&s| Msg::State(s))
            .chain(Emotion::ALL.iter().map(|&e| Msg::Emotion(e)))
            .chain([Msg::Heartbeat]);
        for msg in all {
            assert_eq!(Msg::parse(&msg.to_string()), Some(msg));
        }
    }
}
