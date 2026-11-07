/// Junior — Audio pipeline entry point.
///
/// GPT-4o Audio handles STT + LLM in a single API call.
///
/// Data flow:
///   AudioChunk → [audio_llm_actor] → CommandSequence → [command_executor_actor]
///   CaptureState ←──────────────────────────────────────────────────────────────
///   CaptureState, Thinking, VoiceActivity, EmotionCue, Sleeping → [display_actor] → LED face
use junior_veecle::actors::{
    audio_capture::AudioCaptureActor,
    audio_llm::AudioLlmActor,
    command_executor::CommandExecutorActor,
    display::DisplayActor,
};
use junior_veecle::llm_client::{LocalWakeClient, Mem0Client, MemoryAwareClient, OpenAiClient};

#[veecle_os::osal::std::main(telemetry = false)]
async fn main() {
    dotenvy::dotenv().ok();
    veecle_os::telemetry::info!("Junior starting", pipeline = "audio");

    veecle_os::runtime::execute! {
        store: [
            junior_veecle::types::AudioChunk,
            junior_veecle::types::CommandSequence,
            junior_veecle::types::CaptureState,
            junior_veecle::types::Thinking,
            junior_veecle::types::VoiceActivity,
            junior_veecle::types::EmotionCue,
            junior_veecle::types::Sleeping,
        ],

        actors: [
            AudioCaptureActor,
            AudioLlmActor<MemoryAwareClient<OpenAiClient, Mem0Client, LocalWakeClient>>: MemoryAwareClient {
                audio: OpenAiClient::from_env(),
                memory: Mem0Client::from_env(),
                wake: LocalWakeClient::from_env(),
            },
            CommandExecutorActor,
            DisplayActor,
        ],
    }
    .await;
}
