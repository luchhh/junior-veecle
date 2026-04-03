use serde::{Deserialize, Serialize};
use veecle_os::runtime::Storable;

/// Sequence of robot commands parsed from the LLM tool calls.
#[derive(Debug, Clone, Default, Storable, Serialize, Deserialize)]
pub struct CommandSequence {
    pub commands: Vec<RobotCommand>,
    pub seq: u64,
}

/// A single robot command as defined by the LLM tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "lowercase")]
pub enum RobotCommand {
    Speak { body: String },
    Forward { cm: f64 },
    Backward { cm: f64 },
    Left { deg: f64 },
    Right { deg: f64 },
    #[serde(rename = "happy_dance")]
    HappyDance,
    #[serde(rename = "happy_birthday_giorgio")]
    HappyBirthdayGiorgio,
}

impl RobotCommand {
    /// Parse a single LLM tool call into a command.
    ///
    /// Injects the tool name as the `command` serde tag so the existing
    /// enum deserializer handles validation and field parsing.
    pub fn from_tool_call(name: &str, args: &str) -> Result<Self, serde_json::Error> {
        let mut value: serde_json::Value = serde_json::from_str(args)?;
        value["command"] = serde_json::Value::String(name.to_string());
        serde_json::from_value(value)
    }
}
