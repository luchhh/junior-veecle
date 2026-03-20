/// Command executor actor.
///
/// Reads a CommandSequence from the Store, cancels any in-progress movement,
/// then executes each command in order:
///   speak   → pauses mic capture, plays TTS, resumes capture
///   forward / backward / left / right → drives motors for the given duration
///
/// Uses GpioAbstraction and Speaker so the actor is fully hardware-agnostic.
use std::time::Duration;

use veecle_os::runtime::{InitializedReader, Writer};

use crate::osal::gpio::{Gpio, GpioAbstraction as _};
use crate::osal::speaker::Speaker;
use crate::types::{CaptureState, CommandSequence, RobotCommand};

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
                RobotCommand::Forward { ms } => {
                    veecle_os::telemetry::info!("forward", ms = format!("{ms}"));
                    gpio.forward(100.0);
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                    gpio.stop();
                }
                RobotCommand::Backward { ms } => {
                    veecle_os::telemetry::info!("backward", ms = format!("{ms}"));
                    gpio.reverse(100.0);
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                    gpio.stop();
                }
                RobotCommand::Left { ms } => {
                    veecle_os::telemetry::info!("left", ms = format!("{ms}"));
                    gpio.left_turn(100.0);
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                    gpio.stop();
                }
                RobotCommand::Right { ms } => {
                    veecle_os::telemetry::info!("right", ms = format!("{ms}"));
                    gpio.right_turn(100.0);
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                    gpio.stop();
                }
            }
        }
    }
}
