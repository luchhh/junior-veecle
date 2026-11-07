/// LED face actor.
///
/// Derives what the robot is doing from the Store and mirrors it on the
/// STM32 LED matrix (see `junior_protocol`):
///   speaking (CaptureState.paused) > thinking > listening (VAD) > idle
///
/// The current state is re-sent every 500 ms, which doubles as the display's
/// heartbeat and restores the face after the STM32 resets. Each new
/// EmotionCue is sent once.
use std::time::{Duration, Instant};

use junior_protocol::{Msg, State};
use veecle_os::runtime::Reader;

use crate::osal::display::{Display, DisplayAbstraction as _};
use crate::types::{CaptureState, EmotionCue, Thinking, VoiceActivity};

const POLL_INTERVAL: Duration = Duration::from_millis(50);
const KEEPALIVE_INTERVAL: Duration = Duration::from_millis(500);

pub fn display_state(speaking: bool, thinking: bool, voice: bool) -> State {
    if speaking {
        State::Speaking
    } else if thinking {
        State::Thinking
    } else if voice {
        State::Listening
    } else {
        State::Idle
    }
}

#[veecle_os::runtime::actor]
pub async fn display_actor(
    capture_state: Reader<'_, CaptureState>,
    thinking: Reader<'_, Thinking>,
    voice: Reader<'_, VoiceActivity>,
    emotion: Reader<'_, EmotionCue>,
) -> std::convert::Infallible {
    let mut display = Display::new();
    let mut sent_state = None;
    let mut last_sent = Instant::now();
    let mut emotion_seq = 0u64;

    loop {
        let state = display_state(
            capture_state.read(|s: Option<&CaptureState>| s.is_some_and(|s| s.paused)),
            thinking.read(|t: Option<&Thinking>| t.is_some_and(|t| t.active)),
            voice.read(|v: Option<&VoiceActivity>| v.is_some_and(|v| v.active)),
        );
        if sent_state != Some(state) || last_sent.elapsed() >= KEEPALIVE_INTERVAL {
            display.send(Msg::State(state));
            sent_state = Some(state);
            last_sent = Instant::now();
        }

        let cue = emotion.read(|c: Option<&EmotionCue>| c.cloned());
        if let Some(EmotionCue { emotion: Some(emotion), seq }) = cue {
            if seq != emotion_seq {
                display.send(Msg::Emotion(emotion));
                emotion_seq = seq;
            }
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
