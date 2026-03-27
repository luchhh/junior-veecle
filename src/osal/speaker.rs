/// Speaker abstraction — OSAL extension for USB audio output.
///
/// Backend is selected at runtime via the TTS_BACKEND environment variable:
///   TTS_BACKEND=piper   (default) — local Piper neural TTS + aplay
///   TTS_BACKEND=openai            — OpenAI TTS API + aplay
///
/// On non-Linux platforms (dev) a mock backend is used that calls the
/// system `say` command on macOS or just prints the text.
use tokio::io::AsyncWriteExt as _;

#[cfg(target_os = "linux")]
use crate::audio_device::get_audio_device;

pub enum Speaker {
    Piper {
        model_path: String,
        audio_device: u32,
    },
    OpenAi {
        client: reqwest::Client,
        api_key: String,
        audio_device: u32,
    },
    Mock,
}

impl Speaker {
    /// Build a Speaker from environment variables.
    /// Reads TTS_BACKEND (piper|openai) and OPENAI_API_KEY / PIPER_MODEL.
    pub fn from_env() -> Self {
        match std::env::var("TTS_BACKEND").as_deref() {
            Ok("openai") => {
                let api_key =
                    std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
                Self::OpenAi {
                    client: reqwest::Client::new(),
                    api_key,
                    #[cfg(target_os = "linux")]
                    audio_device: get_audio_device(),
                    #[cfg(not(target_os = "linux"))]
                    audio_device: 0,
                }
            }
            _ => {
                #[cfg(target_os = "linux")]
                return Self::Piper {
                    model_path: std::env::var("PIPER_MODEL").unwrap_or_else(|_| {
                        "/home/pi/piper-voices/en_US-lessac-medium.onnx".into()
                    }),
                    audio_device: get_audio_device(),
                };
                #[cfg(not(target_os = "linux"))]
                Self::Mock
            }
        }
    }

    pub async fn speak(&self, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        println!("[Speaker] Speaking: {text}");

        match self {
            Self::Piper { model_path, audio_device } => {
                speak_piper(text, model_path, *audio_device).await
            }
            Self::OpenAi { client, api_key, audio_device } => {
                speak_openai(client, api_key, text, *audio_device).await
            }
            Self::Mock => speak_mock(text).await,
        }
    }
}

// ── Piper backend ─────────────────────────────────────────────────────────────

async fn speak_piper(text: &str, model_path: &str, audio_device: u32) {
    let speech_path = "/tmp/robot_speech.wav";

    // piper reads text from stdin and writes WAV to the output file.
    let mut child = match tokio::process::Command::new("piper")
        .args(["--model", model_path, "--output_file", speech_path])
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[Speaker] Failed to spawn piper: {e}");
            return;
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes()).await;
    }

    if let Err(e) = child.wait().await {
        eprintln!("[Speaker] piper error: {e}");
        return;
    }

    play_wav(speech_path, audio_device).await;
}

// ── OpenAI TTS backend ────────────────────────────────────────────────────────

async fn speak_openai(
    client: &reqwest::Client,
    api_key: &str,
    text: &str,
    audio_device: u32,
) {
    let speech_path = "/tmp/robot_speech.wav";

    let body = serde_json::json!({
        "model": "tts-1",
        "voice": "alloy",
        "input": text,
        "response_format": "wav",
    });

    let resp = client
        .post("https://api.openai.com/v1/audio/speech")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await;

    match resp {
        Ok(r) => match r.bytes().await {
            Ok(bytes) => {
                if let Err(e) = tokio::fs::write(speech_path, &bytes).await {
                    eprintln!("[Speaker] Failed to write WAV: {e}");
                    return;
                }
                play_wav(speech_path, audio_device).await;
            }
            Err(e) => eprintln!("[Speaker] OpenAI TTS read error: {e}"),
        },
        Err(e) => eprintln!("[Speaker] OpenAI TTS request error: {e}"),
    }
}

// ── Shared playback ───────────────────────────────────────────────────────────

async fn play_wav(path: &str, _audio_device: u32) {
    #[cfg(target_os = "linux")]
    let cmd = tokio::process::Command::new("aplay")
        .args(["-D", &format!("plughw:{_audio_device},0"), path])
        .status()
        .await;

    #[cfg(not(target_os = "linux"))]
    let cmd = tokio::process::Command::new("afplay").arg(path).status().await;

    if let Err(e) = cmd {
        eprintln!("[Speaker] playback error: {e}");
    }
}

// ── Mock backend ──────────────────────────────────────────────────────────────

async fn speak_mock(text: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = tokio::process::Command::new("say").arg(text).status().await;
    }
    #[cfg(not(target_os = "macos"))]
    {
        println!("[MockSpeaker] {text}");
    }
}
