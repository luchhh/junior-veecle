use crate::types::RobotCommand;

pub type ClientError = Box<dyn std::error::Error + Send + Sync>;

const SYSTEM_PROMPT: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/prompts/system.md"));

const TOOLS: &str = r#"[
  {
    "type": "function",
    "function": {
      "name": "heard",
      "description": "Always call this first, every turn, with an exact transcript of what the user just said, in the language they spoke.",
      "parameters": {
        "type": "object",
        "properties": {
          "text": { "type": "string", "description": "What the user said, word for word." }
        },
        "required": ["text"],
        "additionalProperties": false
      },
      "strict": true
    }
  },
  {
    "type": "function",
    "function": {
      "name": "speak",
      "description": "Say something out loud.",
      "parameters": {
        "type": "object",
        "properties": {
          "body": { "type": "string", "description": "Text to say." }
        },
        "required": ["body"],
        "additionalProperties": false
      },
      "strict": true
    }
  },
  {
    "type": "function",
    "function": {
      "name": "forward",
      "description": "Move forward for a given duration.",
      "parameters": {
        "type": "object",
        "properties": {
          "secs": { "type": "number", "description": "Duration in seconds." }
        },
        "required": ["secs"],
        "additionalProperties": false
      },
      "strict": true
    }
  },
  {
    "type": "function",
    "function": {
      "name": "backward",
      "description": "Move backward for a given duration.",
      "parameters": {
        "type": "object",
        "properties": {
          "secs": { "type": "number", "description": "Duration in seconds." }
        },
        "required": ["secs"],
        "additionalProperties": false
      },
      "strict": true
    }
  },
  {
    "type": "function",
    "function": {
      "name": "left",
      "description": "Spin left (counterclockwise) by a given angle.",
      "parameters": {
        "type": "object",
        "properties": {
          "deg": { "type": "number", "description": "Angle in degrees." }
        },
        "required": ["deg"],
        "additionalProperties": false
      },
      "strict": true
    }
  },
  {
    "type": "function",
    "function": {
      "name": "right",
      "description": "Spin right (clockwise) by a given angle.",
      "parameters": {
        "type": "object",
        "properties": {
          "deg": { "type": "number", "description": "Angle in degrees." }
        },
        "required": ["deg"],
        "additionalProperties": false
      },
      "strict": true
    }
  },
  {
    "type": "function",
    "function": {
      "name": "happy_dance",
      "description": "Express happiness or excitement with a celebratory wiggle. Use it spontaneously whenever something good happens, a compliment is given, or the mood calls for it — don't wait to be asked.",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": [],
        "additionalProperties": false
      },
      "strict": true
    }
  },
  {
    "type": "function",
    "function": {
      "name": "happy_birthday_giorgio",
      "description": "Play a special happy birthday tango song for Giorgio. Use it proactively whenever Giorgio's birthday is mentioned or celebrated — don't wait to be explicitly asked.",
      "parameters": {
        "type": "object",
        "properties": {},
        "required": [],
        "additionalProperties": false
      },
      "strict": true
    }
  },
  {
    "type": "function",
    "function": {
      "name": "set_emotion",
      "description": "Show an emotion on your LED face for a few seconds. Use it often and spontaneously to react to the conversation, usually as the first tool call: happy for good news or compliments, sad for bad news, surprised for unexpected things, angry when insulted, love for affection.",
      "parameters": {
        "type": "object",
        "properties": {
          "emotion": {
            "type": "string",
            "enum": ["happy", "sad", "surprised", "angry", "love"],
            "description": "Emotion to show."
          }
        },
        "required": ["emotion"],
        "additionalProperties": false
      },
      "strict": true
    }
  }
]"#;

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

/// Bundles an [AudioPrompt] client with a [MemoryClient], implementing both traits
/// by delegating to the respective inner clients.
pub struct MemoryAwareClient<A, M> {
    pub audio: A,
    pub memory: M,
}

impl<A: AudioPrompt, M: MemoryClient> AudioPrompt for MemoryAwareClient<A, M> {
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

impl<A: AudioPrompt, M: MemoryClient> MemoryClient for MemoryAwareClient<A, M> {
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
        body: serde_json::Value,
    ) -> Result<Vec<RobotCommand>, ClientError> {
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

        let tool_calls = json["choices"][0]["message"]["tool_calls"]
            .as_array()
            .ok_or_else(|| {
                eprintln!("[AudioLLM] unexpected response: {json}");
                "no tool_calls in response"
            })?;

        let commands = tool_calls
            .iter()
            .map(|tc| {
                let name = tc["function"]["name"].as_str().unwrap_or("");
                let args = tc["function"]["arguments"].as_str().unwrap_or("{}");
                RobotCommand::from_tool_call(name, args)
                    .map_err(|e| Box::new(e) as ClientError)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(commands)
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

        let tools: serde_json::Value = serde_json::from_str(TOOLS).unwrap();

        let system_content = if memories.is_empty() {
            SYSTEM_PROMPT.to_string()
        } else {
            format!("{}\n\n## What you remember:\n{}", SYSTEM_PROMPT, memories.join("\n"))
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
            "tools": tools,
            "tool_choice": "required",
            "temperature": 0.7,
        }))
        .await
    }
}

impl TextPrompt for OpenAiClient {
    async fn ask(&self, text: &str) -> Result<Vec<RobotCommand>, ClientError> {
        let tools: serde_json::Value = serde_json::from_str(TOOLS).unwrap();

        self.chat_completion(serde_json::json!({
            "model": "gpt-4o-mini",
            "messages": [
                { "role": "system", "content": SYSTEM_PROMPT },
                { "role": "user", "content": text },
            ],
            "tools": tools,
            "tool_choice": "required",
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
