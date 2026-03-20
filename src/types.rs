use serde::{Deserialize, Serialize};
use veecle_os::runtime::Storable;

/// Audio segment captured from the microphone, preprocessed to 16 kHz mono f32.
#[derive(Debug, Clone, Default, Storable, Serialize, Deserialize)]
pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    /// Monotonic counter — lets downstream actors detect each new chunk.
    pub seq: u64,
}

/// Transcribed text produced by the STT stage.
#[derive(Debug, Clone, Default, Storable, Serialize, Deserialize)]
pub struct TranscribedText {
    pub text: String,
    pub seq: u64,
}

/// Sequence of robot commands parsed from the LLM JSON response.
#[derive(Debug, Clone, Default, Storable, Serialize, Deserialize)]
pub struct CommandSequence {
    pub commands: Vec<RobotCommand>,
    pub seq: u64,
}

/// Signals whether audio capture should be paused (e.g. during TTS playback).
#[derive(Debug, Clone, Default, PartialEq, Storable, Serialize, Deserialize)]
pub struct CaptureState {
    pub paused: bool,
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
