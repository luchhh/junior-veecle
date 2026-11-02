# Memory

Junior should accumulate memories over time — episodic events, facts about the user, derived observations — and use them to behave like a real pet that knows you.

Two distinct problems:

- **Session context** — remembering what was said earlier in the same conversation
- **Long-term memory** — remembering facts and events across sessions, indefinitely

---

## Architecture

### Session context

A `Vec<Message>` held in the actor. Each turn appends the `speak` output from the assistant as a text message. The current turn sends the full history plus the new audio chunk. When the session ends, the history is discarded.

Previous audio is never re-sent — only the text Junior produced. This keeps history cheap.

### Long-term memory

**mem0** (self-hosted, Python sidecar on the Pi) backed by **Chroma** as the local vector store, using **OpenAI embeddings** (`text-embedding-3-small`).

mem0 handles two responsibilities:
- **Extraction** — after each turn, send the conversation to mem0; it uses the LLM to extract facts worth remembering and stores them in Chroma
- **Retrieval** — before each turn, search Chroma for memories relevant to the current context; inject them into the system prompt

All data lives on the Pi. Only the embedding API calls leave (text sent to OpenAI to generate vectors — same privacy bar as the existing LLM calls).

### Memory retrieval timing

The audio model (`gpt-audio`) does not return a transcript of the input. To search memories semantically you need text, but text is only available *after* the call (the `speak` output).

Solution: search using the **previous turn's speak output** as the query. One turn of lag, imperceptible in conversation.

### Prompt structure per turn

```
system:    SYSTEM_PROMPT + "\n\n## What you remember:\n" + top_k_memories   ← rebuilt each turn
assistant: "Hola! ¿En qué te puedo ayudar?"                                  ← previous turns (text only)
user:      [{ type: text, text: session_context }, { type: audio, ... }]     ← current turn
```

The system prompt is rebuilt on every turn. Sending the same memories repeatedly is fine — OpenAI prompt caching handles repeated prefixes at 50% cost.

### Audio + text in the same call

The OpenAI chat completions API supports mixed content arrays. Memory context and session history are injected as text items alongside the audio input in the same API call. No extra round trips.

---

## What we are not doing (now)

**Local embeddings via Ollama** — `nomic-embed-text` would make embeddings fully air-gapped, but adds Ollama as a dependency and is unnecessary given the existing OpenAI relationship. Revisit if privacy requirements change.

**Neo4j graph store** — mem0 supports an optional graph layer for relationship reasoning ("Giorgio is Luis's friend"). Skip for now, start with Chroma only.

**OpenAI Assistants API threads** — would delegate session history to OpenAI, but audio input is not supported on that endpoint, and history would live on the provider.

**Transcribe tool hack** — forcing gpt-audio to call a `transcribe(text)` tool first to expose what it heard. Works but is semantically wrong and may confuse the model. Skip.

**Cloud Whisper before main call** — calling the Whisper API first to get a transcript for same-turn memory search. Fast (< 1s) and cheap ($0.006/min), but adds an extra API call per turn. Not worth it at this stage.

---

## Future

- **Local embeddings** — swap OpenAI embeddings for Ollama + `nomic-embed-text` for full air-gap
- **Graph store** — add Neo4j to mem0 for richer relationship reasoning across memories
- **Same-turn transcript** — cloud Whisper before the main call enables searching memories with the *current* turn's content rather than the previous one
- **Memory inspection UI** — mem0 exposes `get_all()`, could surface this in the existing mic volume web UI
- **Memory pruning** — deduplicate and consolidate memories periodically as they accumulate over years
