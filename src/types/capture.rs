use serde::{Deserialize, Serialize};
use veecle_os::runtime::Storable;

/// Signals whether audio capture should be paused (e.g. during TTS playback).
#[derive(Debug, Clone, Default, PartialEq, Storable, Serialize, Deserialize)]
pub struct CaptureState {
    pub paused: bool,
}
