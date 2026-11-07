/// Junior — Whisper pipeline entry point.
///
/// Data flow:
///   AudioChunk → [stt_actor] → TranscribedText → [llm_actor] → CommandSequence
///                                                              → [command_executor_actor]
///   CaptureState ←──────────────────────────────────────────────────────────────
///   CaptureState, Thinking, VoiceActivity, EmotionCue → [display_actor] → LED face
use junior_veecle::actors::{
    audio_capture::AudioCaptureActor,
    command_executor::CommandExecutorActor,
    display::DisplayActor,
    llm::LlmActor,
    stt::SttActor,
};
use junior_veecle::llm_client::OpenAiClient;

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
            junior_veecle::types::Thinking,
            junior_veecle::types::VoiceActivity,
            junior_veecle::types::EmotionCue,
        ],

        actors: [
            AudioCaptureActor,
            SttActor<OpenAiClient>: OpenAiClient::from_env(),
            LlmActor<OpenAiClient>: OpenAiClient::from_env(),
            CommandExecutorActor,
            DisplayActor,
        ],
    }
    .await;
}
