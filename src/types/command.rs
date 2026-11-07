use junior_protocol::Emotion;
use serde::{Deserialize, Serialize};
use veecle_os::runtime::Storable;

/// Sequence of robot commands parsed from the LLM response.
#[derive(Debug, Clone, Default, Storable, Serialize, Deserialize)]
pub struct CommandSequence {
    pub commands: Vec<RobotCommand>,
    pub seq: u64,
}

/// A single robot command, as executed by the command executor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "lowercase")]
pub enum RobotCommand {
    Speak { body: String },
    Forward { secs: f64 },
    Backward { secs: f64 },
    Left { deg: f64 },
    Right { deg: f64 },
    #[serde(rename = "happy_dance")]
    HappyDance,
    #[serde(rename = "happy_birthday_giorgio")]
    HappyBirthdayGiorgio,
    #[serde(rename = "set_emotion")]
    SetEmotion { emotion: Emotion },
}

/// Arguments of the `respond` tool: the model's whole answer for one turn.
#[derive(Debug, Deserialize)]
struct Response {
    say: String,
    /// One of the emotions, or "none".
    emotion: String,
    actions: Vec<Action>,
}

#[derive(Debug, Deserialize)]
struct Action {
    #[serde(rename = "type")]
    kind: ActionKind,
    /// Seconds for forward/backward, degrees for left/right, unused otherwise.
    value: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ActionKind {
    Forward,
    Backward,
    Left,
    Right,
    HappyDance,
    HappyBirthdayGiorgio,
}

impl RobotCommand {
    /// Turns the `respond` tool arguments into commands: the emotion first, so the face
    /// reacts right away, then what to say, then the actions in order. Empty parts
    /// ("" to say, "none" emotion, no actions) produce no command.
    pub fn from_response(args: &str) -> Result<Vec<Self>, serde_json::Error> {
        let response: Response = serde_json::from_str(args)?;
        let mut commands = Vec::with_capacity(response.actions.len() + 2);

        if response.emotion != "none" {
            let emotion = serde_json::from_value(serde_json::Value::String(response.emotion))?;
            commands.push(RobotCommand::SetEmotion { emotion });
        }
        let say = response.say.trim();
        if !say.is_empty() {
            commands.push(RobotCommand::Speak { body: say.to_owned() });
        }
        commands.extend(response.actions.into_iter().map(|action| match action.kind {
            ActionKind::Forward => RobotCommand::Forward { secs: action.value },
            ActionKind::Backward => RobotCommand::Backward { secs: action.value },
            ActionKind::Left => RobotCommand::Left { deg: action.value },
            ActionKind::Right => RobotCommand::Right { deg: action.value },
            ActionKind::HappyDance => RobotCommand::HappyDance,
            ActionKind::HappyBirthdayGiorgio => RobotCommand::HappyBirthdayGiorgio,
        }));

        Ok(commands)
    }
}
