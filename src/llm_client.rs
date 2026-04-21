use crate::types::RobotCommand;

pub type ClientError = Box<dyn std::error::Error + Send + Sync>;

const SYSTEM_PROMPT: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/prompts/system.md"));

const TOOLS: &str = r#"[
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
      "description": "Move forward a given distance.",
      "parameters": {
        "type": "object",
        "properties": {
          "cm": { "type": "number", "description": "Distance in centimeters." }
        },
        "required": ["cm"],
        "additionalProperties": false
      },
      "strict": true
    }
  },
  {
    "type": "function",
    "function": {
      "name": "backward",
      "description": "Move backward a given distance.",
      "parameters": {
        "type": "object",
        "properties": {
          "cm": { "type": "number", "description": "Distance in centimeters." }
        },
        "required": ["cm"],
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
  }
]"#;

/// Ask an AI model something using raw audio as input, returns parsed commands.
pub trait AudioPrompt: Send + Sync {
    fn ask(
        &self,
        samples: &[f32],
        sample_rate: u32,
    ) -> impl std::future::Future<Output = Result<Vec<RobotCommand>, ClientError>> + Send;
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

        let tool_calls = json["choices"][0]["message"]["tool_calls"]
            .as_array()
            .ok_or("no tool_calls in response")?;

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
    ) -> Result<Vec<RobotCommand>, ClientError> {
        use base64::Engine as _;

        let audio_b64 =
            base64::engine::general_purpose::STANDARD.encode(encode_wav(samples, sample_rate));

        let tools: serde_json::Value = serde_json::from_str(TOOLS).unwrap();

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
