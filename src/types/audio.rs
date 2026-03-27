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
