use crate::types::RobotCommand;

pub type ClientError = Box<dyn std::error::Error + Send + Sync>;

const SYSTEM_PROMPT: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/prompts/system.md"));

/// The only tool, and the model is forced to call it: with free tool calls gpt-audio often
/// stopped after the first one (e.g. an emotion without the speech). Strict mode makes every
/// field present and valid; empty values mean "nothing this turn".
const RESPOND_TOOL: &str = r#"{
  "type": "function",
  "function": {
    "name": "respond",
    "description": "Your whole response for this turn: what to say, the emotion to show, and the actions to take.",
    "parameters": {
      "type": "object",
      "properties": {
        "say": { "type": "string", "description": "What to say out loud, or empty to stay silent." },
        "emotion": {
          "type": "string",
          "enum": ["none", "happy", "sad", "surprised", "angry", "love"],
          "description": "Emotion to show on your LED face for a few seconds, or none."
        },
        "actions": {
          "type": "array",
          "description": "Actions to take in order, or empty.",
          "items": {
            "type": "object",
            "properties": {
              "type": {
                "type": "string",
                "enum": ["forward", "backward", "left", "right", "happy_dance", "happy_birthday_giorgio", "be_quiet", "sleep"]
              },
              "value": {
                "type": "number",
                "description": "Seconds for forward/backward, degrees for left/right, minutes for be_quiet (0 to 60), 0 for the others."
              }
            },
            "required": ["type", "value"],
            "additionalProperties": false
          }
        }
      },
      "required": ["say", "emotion", "actions"],
      "additionalProperties": false
    },
    "strict": true
  }
}"#;

/// Request fields that make the model answer with exactly one `respond` call.
fn respond_tool_fields() -> serde_json::Value {
    let tool: serde_json::Value = serde_json::from_str(RESPOND_TOOL).unwrap();
    serde_json::json!({
        "tools": [tool],
        "tool_choice": { "type": "function", "function": { "name": "respond" } },
    })
}

/// One exchange of the conversation: what the user said and what Junior said back.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Turn {
    pub user: Option<String>,
    pub junior: Option<String>,
}

impl Turn {
    fn messages(&self) -> Vec<serde_json::Value> {
        let user = self.user.iter().map(|t| serde_json::json!({ "role": "user", "content": t }));
        let junior =
            self.junior.iter().map(|t| serde_json::json!({ "role": "assistant", "content": t }));
        user.chain(junior).collect()
    }
}

/// Ask an AI model something using raw audio as input, returns parsed commands.
pub trait AudioPrompt: Send + Sync {
    fn ask(
        &self,
        samples: &[f32],
        sample_rate: u32,
        history: &[Turn],
        memories: &[String],
    ) -> impl std::future::Future<Output = Result<Vec<RobotCommand>, ClientError>> + Send;
}

/// Store and retrieve long-term memories from an external memory sidecar.
pub trait MemoryClient: Send + Sync {
    /// The most recent `limit` memories, oldest first.
    fn recent(
        &self,
        limit: usize,
    ) -> impl std::future::Future<Output = Result<Vec<String>, ClientError>> + Send;

    /// Hand a conversation turn to the memory store, which extracts the facts worth keeping.
    fn add(
        &self,
        turn: &Turn,
    ) -> impl std::future::Future<Output = Result<(), ClientError>> + Send;
}

/// Transcribes audio on the robot itself, so nothing leaves it while Junior sleeps.
pub trait WakeListener: Send + Sync {
    fn transcribe_locally(
        &self,
        samples: &[f32],
        sample_rate: u32,
    ) -> impl std::future::Future<Output = Result<String, ClientError>> + Send;
}

/// Bundles the cloud client, the [MemoryClient] and the local [WakeListener],
/// implementing all their traits by delegating to the respective inner clients.
pub struct MemoryAwareClient<A, M, W> {
    pub audio: A,
    pub memory: M,
    pub wake: W,
}

impl<A: AudioPrompt, M: MemoryClient, W: Send + Sync> AudioPrompt for MemoryAwareClient<A, M, W> {
    fn ask(
        &self,
        samples: &[f32],
        sample_rate: u32,
        history: &[Turn],
        memories: &[String],
    ) -> impl std::future::Future<Output = Result<Vec<RobotCommand>, ClientError>> + Send {
        self.audio.ask(samples, sample_rate, history, memories)
    }
}

impl<A: Send + Sync, M: MemoryClient, W: Send + Sync> MemoryClient for MemoryAwareClient<A, M, W> {
    fn recent(
        &self,
        limit: usize,
    ) -> impl std::future::Future<Output = Result<Vec<String>, ClientError>> + Send {
        self.memory.recent(limit)
    }

    fn add(
        &self,
        turn: &Turn,
    ) -> impl std::future::Future<Output = Result<(), ClientError>> + Send {
        self.memory.add(turn)
    }
}

impl<A: AudioToText, M: Send + Sync, W: Send + Sync> AudioToText for MemoryAwareClient<A, M, W> {
    fn transcribe(
        &self,
        samples: &[f32],
        sample_rate: u32,
    ) -> impl std::future::Future<Output = Result<String, ClientError>> + Send {
        self.audio.transcribe(samples, sample_rate)
    }
}

impl<A: Send + Sync, M: Send + Sync, W: WakeListener> WakeListener for MemoryAwareClient<A, M, W> {
    fn transcribe_locally(
        &self,
        samples: &[f32],
        sample_rate: u32,
    ) -> impl std::future::Future<Output = Result<String, ClientError>> + Send {
        self.wake.transcribe_locally(samples, sample_rate)
    }
}

/// HTTP client for the local wake-word sidecar (`scripts/wake-server.py`, faster-whisper).
pub struct LocalWakeClient {
    client: reqwest::Client,
    url: String,
}

impl LocalWakeClient {
    pub fn from_env() -> Self {
        Self {
            client: reqwest::Client::new(),
            url: std::env::var("WAKE_URL")
                .unwrap_or_else(|_| "http://localhost:8001/transcribe".to_string()),
        }
    }
}

impl WakeListener for LocalWakeClient {
    async fn transcribe_locally(&self, samples: &[f32], sample_rate: u32) -> Result<String, ClientError> {
        let json = self
            .client
            .post(&self.url)
            .header(reqwest::header::CONTENT_TYPE, "audio/wav")
            .body(encode_wav(samples, sample_rate))
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        Ok(json["text"].as_str().unwrap_or("").trim().to_string())
    }
}

/// mem0 HTTP client — talks to the self-hosted mem0 sidecar on the Pi.
pub struct Mem0Client {
    client: reqwest::Client,
    base_url: String,
    user_id: String,
}

impl Mem0Client {
    pub fn from_env() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: std::env::var("MEM0_URL")
                .unwrap_or_else(|_| "http://localhost:8000".to_string()),
            user_id: std::env::var("MEM0_USER_ID").unwrap_or_else(|_| "junior".to_string()),
        }
    }
}

impl MemoryClient for Mem0Client {
    async fn recent(&self, limit: usize) -> Result<Vec<String>, ClientError> {
        let resp = self
            .client
            .get(format!("{}/v1/memories/", self.base_url))
            .query(&[("user_id", &self.user_id)])
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        // created_at is ISO 8601, so sorting the strings sorts by time.
        let mut results: Vec<&serde_json::Value> =
            resp["results"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
        results.sort_by_key(|r| r["created_at"].as_str().unwrap_or("").to_owned());

        let memories: Vec<String> = results
            .iter()
            .filter_map(|r| r["memory"].as_str().map(str::to_owned))
            .collect();
        let skip = memories.len().saturating_sub(limit);

        Ok(memories.into_iter().skip(skip).collect())
    }

    async fn add(&self, turn: &Turn) -> Result<(), ClientError> {
        self.client
            .post(format!("{}/v1/memories/", self.base_url))
            .json(&serde_json::json!({
                "messages": turn.messages(),
                "user_id": self.user_id,
            }))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

/// Ask an AI model something using text as input, returns parsed commands.
pub trait TextPrompt: Send + Sync {
    fn ask(
        &self,
        text: &str,
    ) -> impl std::future::Future<Output = Result<Vec<RobotCommand>, ClientError>> + Send;
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

    async fn chat_completion(
        &self,
        mut body: serde_json::Value,
    ) -> Result<Vec<RobotCommand>, ClientError> {
        if let (Some(body), serde_json::Value::Object(fields)) =
            (body.as_object_mut(), respond_tool_fields())
        {
            body.extend(fields);
        }

        let json = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        eprintln!("[DEBUG] full response: {}", serde_json::to_string_pretty(&json).unwrap());

        let args = json["choices"][0]["message"]["tool_calls"]
            .as_array()
            .and_then(|calls| calls.iter().find(|c| c["function"]["name"] == "respond"))
            .and_then(|call| call["function"]["arguments"].as_str())
            .ok_or_else(|| {
                eprintln!("[AudioLLM] unexpected response: {json}");
                "no respond call in response"
            })?;

        Ok(RobotCommand::from_response(args)?)
    }
}

impl AudioPrompt for OpenAiClient {
    async fn ask(
        &self,
        samples: &[f32],
        sample_rate: u32,
        history: &[Turn],
        memories: &[String],
    ) -> Result<Vec<RobotCommand>, ClientError> {
        use base64::Engine as _;

        let audio_b64 =
            base64::engine::general_purpose::STANDARD.encode(encode_wav(samples, sample_rate));


        let system_content = if memories.is_empty() {
            SYSTEM_PROMPT.to_string()
        } else {
            format!(
                "{}\n\n## What you remember (oldest first; a later fact overrides an earlier one):\n{}",
                SYSTEM_PROMPT,
                memories.join("\n")
            )
        };

        let mut messages: Vec<serde_json::Value> =
            vec![serde_json::json!({ "role": "system", "content": system_content })];

        messages.extend(history.iter().flat_map(Turn::messages));

        messages.push(serde_json::json!({
            "role": "user",
            "content": [{
                "type": "input_audio",
                "input_audio": { "data": audio_b64, "format": "wav" },
            }],
        }));

        self.chat_completion(serde_json::json!({
            "model": "gpt-audio",
            "messages": messages,
            "temperature": 0.7,
        }))
        .await
    }
}

impl TextPrompt for OpenAiClient {
    async fn ask(&self, text: &str) -> Result<Vec<RobotCommand>, ClientError> {

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
                    // No language hint: Junior is spoken to in English and Spanish.
                    .text("model", "gpt-4o-mini-transcribe"),
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
