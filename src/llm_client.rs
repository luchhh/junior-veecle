pub type ClientError = Box<dyn std::error::Error + Send + Sync>;

const SYSTEM_PROMPT: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/prompts/system.md"));

/// Ask an AI model something using raw audio as input, returns the raw text response.
pub trait AudioPrompt: Send + Sync {
    fn ask(
        &self,
        samples: &[f32],
        sample_rate: u32,
    ) -> impl std::future::Future<Output = Result<String, ClientError>> + Send;
}

/// Ask an AI model something using text as input, returns the raw text response.
pub trait TextPrompt: Send + Sync {
    fn ask(&self, text: &str) -> impl std::future::Future<Output = Result<String, ClientError>> + Send;
}

/// Transcribe raw audio into text.
pub trait AudioToText: Send + Sync {
    fn transcribe(
        &self,
        samples: &[f32],
        sample_rate: u32,
    ) -> impl std::future::Future<Output = Result<String, ClientError>> + Send;
}

/// OpenAI implementation of all client traits.
pub struct OpenAiClient {
    client: reqwest::Client,
    api_key: String,
}

impl OpenAiClient {
    pub fn from_env() -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set"),
        }
    }

    async fn chat_completion(&self, body: serde_json::Value) -> Result<String, ClientError> {
        let json = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        Ok(json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_string())
    }
}

impl AudioPrompt for OpenAiClient {
    async fn ask(&self, samples: &[f32], sample_rate: u32) -> Result<String, ClientError> {
        use base64::Engine as _;

        let audio_b64 =
            base64::engine::general_purpose::STANDARD.encode(encode_wav(samples, sample_rate));

        self.chat_completion(serde_json::json!({
            "model": "gpt-4o-audio-preview",
            "messages": [
                { "role": "system", "content": SYSTEM_PROMPT },
                {
                    "role": "user",
                    "content": [{
                        "type": "input_audio",
                        "input_audio": { "data": audio_b64, "format": "wav" },
                    }],
                },
            ],
            "temperature": 0.7,
        }))
        .await
    }
}

impl TextPrompt for OpenAiClient {
    async fn ask(&self, text: &str) -> Result<String, ClientError> {
        self.chat_completion(serde_json::json!({
            "model": "gpt-4o-mini",
            "messages": [
                { "role": "system", "content": SYSTEM_PROMPT },
                { "role": "user", "content": text },
            ],
            "temperature": 0.7,
        }))
        .await
    }
}

impl AudioToText for OpenAiClient {
    async fn transcribe(&self, samples: &[f32], sample_rate: u32) -> Result<String, ClientError> {
        let part = reqwest::multipart::Part::bytes(encode_wav(samples, sample_rate))
            .file_name("audio.wav")
            .mime_str("audio/wav")?;

        let json = self
            .client
            .post("https://api.openai.com/v1/audio/transcriptions")
            .bearer_auth(&self.api_key)
            .multipart(
                reqwest::multipart::Form::new()
                    .part("file", part)
                    .text("model", "whisper-1")
                    .text("language", "en"),
            )
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        Ok(json["text"].as_str().unwrap_or("").trim().to_string())
    }
}

fn encode_wav(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
    for &s in samples {
        writer
            .write_sample((s * 32_767.0).clamp(-32_768.0, 32_767.0) as i16)
            .unwrap();
    }
    writer.finalize().unwrap();
    cursor.into_inner()
}
