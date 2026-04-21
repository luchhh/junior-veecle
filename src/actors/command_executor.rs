/// Command executor actor.
///
/// Reads a CommandSequence from the Store, cancels any in-progress movement,
/// then executes each command in order:
///   speak   → pauses mic capture, plays TTS, resumes capture
///   forward / backward → drives motors for the given distance in cm
///   left / right       → spins motors for the given angle in degrees
///
/// Speed constants (from physical measurements):
///   LINEAR_SPEED_CM_PER_S  = 10.0 cm/s
///   ROTATION_DEG_PER_S     = 30.0 deg/s
use std::time::Duration;

use veecle_os::runtime::{InitializedReader, Writer};

use crate::osal::gpio::{Gpio, GpioAbstraction as _};
use crate::osal::speaker::Speaker;
use crate::types::{CaptureState, CommandSequence, RobotCommand};

const LINEAR_SPEED_CM_PER_S: f64 = 10.0;
const ROTATION_DEG_PER_S: f64 = 30.0;

fn cm_to_duration(cm: f64) -> Duration {
    Duration::from_secs_f64(cm / LINEAR_SPEED_CM_PER_S)
}

fn deg_to_duration(deg: f64) -> Duration {
    Duration::from_secs_f64(deg / ROTATION_DEG_PER_S)
}

#[veecle_os::runtime::actor]
pub async fn command_executor_actor(
    mut commands_in: InitializedReader<'_, CommandSequence>,
    mut capture_state_out: Writer<'_, CaptureState>,
) -> std::convert::Infallible {
    let mut gpio = Gpio::new();
    let speaker = Speaker::from_env();

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
                RobotCommand::Forward { cm } => {
                    veecle_os::telemetry::info!("forward", cm = format!("{cm}"));
                    gpio.forward(100.0);
                    tokio::time::sleep(cm_to_duration(cm)).await;
                    gpio.stop();
                }
                RobotCommand::Backward { cm } => {
                    veecle_os::telemetry::info!("backward", cm = format!("{cm}"));
                    gpio.reverse(100.0);
                    tokio::time::sleep(cm_to_duration(cm)).await;
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
                        tokio::time::sleep(cm_to_duration(10.0)).await;
                        gpio.reverse(100.0);
                        tokio::time::sleep(cm_to_duration(10.0)).await;
                        gpio.forward(100.0);
                        tokio::time::sleep(cm_to_duration(20.0)).await;
                    }
                    gpio.stop();
                }
                RobotCommand::HappyBirthdayGiorgio => {
                    veecle_os::telemetry::info!("happy_birthday_giorgio");
                    capture_state_out.write(CaptureState { paused: true }).await;
                    speaker.play_wav_file("assets/happy-birthday-giorgio.wav").await;
                    capture_state_out.write(CaptureState { paused: false }).await;
                }
            }
        }
    }
}
