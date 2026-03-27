use serde::{Deserialize, Serialize};
use veecle_os::runtime::Storable;

/// Sequence of robot commands parsed from the LLM JSON response.
#[derive(Debug, Clone, Default, Storable, Serialize, Deserialize)]
pub struct CommandSequence {
    pub commands: Vec<RobotCommand>,
    pub seq: u64,
}

/// A single robot command as defined in prompts/system.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "lowercase")]
pub enum RobotCommand {
    Speak { body: String },
    Forward { ms: u64 },
    Backward { ms: u64 },
    Left { ms: u64 },
    Right { ms: u64 },
}

impl RobotCommand {
    /// Parse a JSON string into a list of commands.
    pub fn parse_many(s: &str) -> Result<Vec<Self>, serde_json::Error> {
        serde_json::from_str(s.trim())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_command_variants() {
        let json = r#"[
            {"command": "speak",    "body": "hello"},
            {"command": "forward",  "ms": 1000},
            {"command": "backward", "ms": 500},
            {"command": "left",     "ms": 333},
            {"command": "right",    "ms": 666}
        ]"#;
        let commands = RobotCommand::parse_many(json).unwrap();
        assert!(matches!(&commands[0], RobotCommand::Speak { body } if body == "hello"));
        assert!(matches!(commands[1], RobotCommand::Forward  { ms: 1000 }));
        assert!(matches!(commands[2], RobotCommand::Backward { ms: 500  }));
        assert!(matches!(commands[3], RobotCommand::Left     { ms: 333  }));
        assert!(matches!(commands[4], RobotCommand::Right    { ms: 666  }));
    }

    #[test]
    fn parses_empty_command_list() {
        let commands = RobotCommand::parse_many("[]").unwrap();
        assert!(commands.is_empty());
    }

    #[test]
    fn trims_whitespace_before_parsing() {
        let json = "  \n[{\"command\": \"forward\", \"ms\": 100}]\n  ";
        let commands = RobotCommand::parse_many(json).unwrap();
        assert_eq!(commands.len(), 1);
    }

    #[test]
    fn returns_error_on_invalid_json() {
        assert!(RobotCommand::parse_many("not json").is_err());
    }

    #[test]
    fn returns_error_on_unknown_command() {
        assert!(RobotCommand::parse_many(r#"[{"command": "fly", "ms": 100}]"#).is_err());
    }
}
