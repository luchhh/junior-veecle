/// Command executor actor.
///
/// Reads a CommandSequence from the Store, cancels any in-progress movement,
/// then executes each command in order:
///   speak   → pauses mic capture, plays TTS, resumes capture
///   forward / backward → drives motors for the given duration in seconds
///   left / right       → spins motors for the given angle in degrees
///   set_emotion        → publishes an EmotionCue for the LED face
///   be_quiet           → skips every speak for the given minutes (at most an hour)
///
/// Speed constants (from physical measurements):
///   ROTATION_DEG_PER_S     = 30.0 deg/s
use std::time::{Duration, Instant};

use veecle_os::runtime::{InitializedReader, Writer};

use crate::osal::gpio::{Gpio, GpioAbstraction as _};
use crate::osal::speaker::Speaker;
use crate::types::{CaptureState, CommandSequence, EmotionCue, RobotCommand};

const ROTATION_DEG_PER_S: f64 = 30.0;
/// Longest `be_quiet` accepted; longer requests are cut to this.
pub const MAX_QUIET: Duration = Duration::from_secs(60 * 60);

fn deg_to_duration(deg: f64) -> Duration {
    Duration::from_secs_f64(deg / ROTATION_DEG_PER_S)
}

/// Tracks a `be_quiet` request: until when `speak` is skipped.
#[derive(Debug, Default)]
pub struct Quiet {
    until: Option<Instant>,
}

impl Quiet {
    /// Starts (or with 0, ends) a quiet period at `now` and returns its length after capping.
    pub fn start(&mut self, minutes: f64, now: Instant) -> Duration {
        // Also maps NaN and negative values to 0 and infinity to the cap.
        let duration = if minutes > 0.0 {
            Duration::from_secs_f64(minutes.min(60.0) * 60.0).min(MAX_QUIET)
        } else {
            Duration::ZERO
        };
        self.until = (!duration.is_zero()).then(|| now + duration);
        duration
    }

    pub fn is_quiet(&self, now: Instant) -> bool {
        self.until.is_some_and(|until| now < until)
    }
}

#[veecle_os::runtime::actor]
pub async fn command_executor_actor(
    mut commands_in: InitializedReader<'_, CommandSequence>,
    mut capture_state_out: Writer<'_, CaptureState>,
    mut emotion_out: Writer<'_, EmotionCue>,
) -> std::convert::Infallible {
    let mut gpio = Gpio::new();
    let speaker = Speaker::from_env();
    let mut emotion_seq = 0u64;
    let mut quiet = Quiet::default();

    loop {
        let sequence = commands_in.wait_for_update().await.read_cloned();

        // Cancel any in-progress movement before executing new commands —
        // mirrors Python's firmware.clear() call in Robot._handle_response().
        gpio.stop();

        veecle_os::telemetry::info!(
            "Executing commands",
            count = format!("{}", sequence.commands.len())
        );

        for cmd in sequence.commands {
            match cmd {
                RobotCommand::Speak { body } if quiet.is_quiet(Instant::now()) => {
                    println!("[Executor] quiet, not saying: {body:?}");
                }
                RobotCommand::Speak { body } => {
                    // Pause capture so the robot doesn't hear its own voice.
                    capture_state_out
                        .write(CaptureState { paused: true })
                        .await;
                    speaker.speak(&body).await;
                    capture_state_out
                        .write(CaptureState { paused: false })
                        .await;
                }
                RobotCommand::Forward { secs } => {
                    veecle_os::telemetry::info!("forward", secs = format!("{secs}"));
                    gpio.forward(100.0);
                    tokio::time::sleep(Duration::from_secs_f64(secs)).await;
                    gpio.stop();
                }
                RobotCommand::Backward { secs } => {
                    veecle_os::telemetry::info!("backward", secs = format!("{secs}"));
                    gpio.reverse(100.0);
                    tokio::time::sleep(Duration::from_secs_f64(secs)).await;
                    gpio.stop();
                }
                RobotCommand::Left { deg } => {
                    veecle_os::telemetry::info!("left", deg = format!("{deg}"));
                    gpio.left_turn(100.0);
                    tokio::time::sleep(deg_to_duration(deg)).await;
                    gpio.stop();
                }
                RobotCommand::Right { deg } => {
                    veecle_os::telemetry::info!("right", deg = format!("{deg}"));
                    gpio.right_turn(100.0);
                    tokio::time::sleep(deg_to_duration(deg)).await;
                    gpio.stop();
                }
                RobotCommand::HappyDance => {
                    veecle_os::telemetry::info!("happy_dance");
                    for _ in 0..2 {
                        gpio.reverse(100.0);
                        tokio::time::sleep(Duration::from_secs_f64(1.0)).await;
                        gpio.reverse(100.0);
                        tokio::time::sleep(Duration::from_secs_f64(1.0)).await;
                        gpio.forward(100.0);
                        tokio::time::sleep(Duration::from_secs_f64(2.0)).await;
                    }
                    gpio.stop();
                }
                RobotCommand::HappyBirthdayGiorgio => {
                    veecle_os::telemetry::info!("happy_birthday_giorgio");
                    capture_state_out.write(CaptureState { paused: true }).await;
                    speaker.play_wav_file("assets/happy-birthday-giorgio.wav").await;
                    capture_state_out.write(CaptureState { paused: false }).await;
                }
                RobotCommand::SetEmotion { emotion } => {
                    veecle_os::telemetry::info!("set_emotion", emotion = emotion.as_str());
                    emotion_seq += 1;
                    emotion_out
                        .write(EmotionCue { emotion: Some(emotion), seq: emotion_seq })
                        .await;
                }
                RobotCommand::BeQuiet { minutes } => {
                    let duration = quiet.start(minutes, Instant::now());
                    println!("[Executor] quiet for {}s", duration.as_secs());
                }
            }
        }
    }
}
