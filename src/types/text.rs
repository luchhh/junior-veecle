use serde::{Deserialize, Serialize};
use veecle_os::runtime::Storable;

/// Transcribed text produced by the STT stage.
#[derive(Debug, Clone, Default, Storable, Serialize, Deserialize)]
pub struct TranscribedText {
    pub text: String,
    pub seq: u64,
}
