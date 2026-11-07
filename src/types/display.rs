use junior_protocol::Emotion;
use serde::{Deserialize, Serialize};
use veecle_os::runtime::Storable;

/// Whether the mic's VAD currently hears someone talking.
#[derive(Debug, Clone, Default, PartialEq, Storable, Serialize, Deserialize)]
pub struct VoiceActivity {
    pub active: bool,
}

/// Whether a request to the AI model is in flight.
#[derive(Debug, Clone, Default, PartialEq, Storable, Serialize, Deserialize)]
pub struct Thinking {
    pub active: bool,
}

/// Whether Junior is asleep: it only listens, locally, for the wake-up phrase.
#[derive(Debug, Clone, Default, PartialEq, Storable, Serialize, Deserialize)]
pub struct Sleeping {
    pub active: bool,
}

/// Latest emotion requested by the AI model, for the LED face.
#[derive(Debug, Clone, Default, Storable, Serialize, Deserialize)]
pub struct EmotionCue {
    pub emotion: Option<Emotion>,
    /// Monotonic counter, so repeating the same emotion still shows it again.
    pub seq: u64,
}
