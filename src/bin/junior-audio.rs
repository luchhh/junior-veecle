/// Junior — Audio pipeline entry point.
///
/// GPT-4o Audio handles STT + LLM in a single API call.
///
/// Data flow:
///   AudioChunk → [audio_llm_actor] → CommandSequence → [command_executor_actor]
///   CaptureState ←──────────────────────────────────────────────────────────────
use junior_veecle::actors::{
    audio_capture::AudioCaptureActor,
    audio_llm::AudioLlmActor,
    command_executor::CommandExecutorActor,
};

#[veecle_os::osal::std::main(telemetry = false)]
async fn main() {
    dotenvy::dotenv().ok();
    veecle_os::telemetry::info!("Junior starting", pipeline = "audio");

    veecle_os::runtime::execute! {
        store: [
            junior_veecle::types::AudioChunk,
            junior_veecle::types::CommandSequence,
            junior_veecle::types::CaptureState,
        ],

        actors: [
            AudioCaptureActor,
            AudioLlmActor,
            CommandExecutorActor,
        ],
    }
    .await;
}
