/// Junior — Whisper pipeline entry point.
///
/// Data flow:
///   AudioChunk → [stt_actor] → TranscribedText → [llm_actor] → CommandSequence
///                                                              → [command_executor_actor]
///   CaptureState ←──────────────────────────────────────────────────────────────
use junior_veecle::actors::{
    audio_capture::AudioCaptureActor,
    command_executor::CommandExecutorActor,
    llm::LlmActor,
    stt::SttActor,
};

#[veecle_os::osal::std::main(telemetry = true)]
async fn main() {
    dotenvy::dotenv().ok();
    veecle_os::telemetry::info!("Junior starting", pipeline = "whisper");

    veecle_os::runtime::execute! {
        store: [
            junior_veecle::types::AudioChunk,
            junior_veecle::types::TranscribedText,
            junior_veecle::types::CommandSequence,
            junior_veecle::types::CaptureState,
        ],

        actors: [
            AudioCaptureActor,
            SttActor,
            LlmActor,
            CommandExecutorActor,
        ],
    }
    .await;
}
