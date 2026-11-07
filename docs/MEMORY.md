# Memory

Junior should remember what you tell it, like a pet that knows you. Memory has two levels:

- **Session** — the current conversation, word for word, for the last hour.
- **Long-term** — lasting facts about the user, their home, the people in their life and their preferences, kept indefinitely.

Recent context and lasting facts need different handling: what was said a minute ago matters
word for word but only for a while, while a fact ("the floor is tile, not wood") matters forever
but only in its final, corrected form.

---

## How it works today

### Each turn

```
audio chunk ──┬─> gpt-audio (forced `respond` tool) ──> commands ──> robot
              └─> gpt-4o-mini-transcribe ──────────────> transcript ─┐
                                                                      ├─> session + mem0
                         Junior's `say` from the response ───────────┘
```

- The audio goes to `gpt-audio` and, in parallel, to `gpt-4o-mini-transcribe`. The transcript is
  awaited only after the commands are sent, so it never delays the reply (it is usually, but not
  always, faster).
- The prompt carries the long-term memories (oldest first; "a later fact overrides an earlier one")
  and the session turns as user/assistant messages.

### Session

`SessionHistory` in `src/actors/audio_llm.rs`: the turns of the last 60 minutes, at most 30, held in
the `junior` process. A restart starts an empty session.

### Long-term

mem0 (Python sidecar, `scripts/mem0-server.py`, Chroma + OpenAI embeddings). Each turn is sent to
mem0, whose LLM (`gpt-4o-mini`) extracts facts. `custom_instructions` ask it to keep only lasting
facts. Each turn loads the newest 100 memories; there is no search.

### Sleep

While Junior sleeps, audio is only transcribed on the robot (`scripts/wake-server.py`) to listen
for the wake-up phrase. Nothing reaches the session or long-term memory.

---

## Lessons learned

- **Don't make the model report what it heard.** A `heard` tool was unreliable: `gpt-audio` often
  answers with a single tool call, so `heard` either replaced the reply (Junior went mute) or was
  dropped (6/6 → 3/6 once another tool was added). A separate transcription call is deterministic.
- **Free tool calls are unreliable with `gpt-audio`.** It frequently stopped after one call
  (an emotion without the speech, the speech without the movement). One forced `respond` tool in
  strict mode (`say`, `emotion`, `actions[]`) fixed it. `response_format: json_schema` is not
  supported by `gpt-audio`.
- **Searching memories needs the current question as text**, which only exists after the reply.
  Searching with the previous reply found the wrong things. With few memories, sending all of them
  is simpler and always right.
- **mem0 2.0 limits:**
  - `get_all` returns 20 memories by default; newer ones were invisible until `top_k` was raised.
  - It is ADD-only: corrections and rewordings pile up next to the old version
    ("wooden floor" stays next to "tile floor"; the same fact appears three times).
  - Its extraction prompt (~33k chars) asks to capture everything, including plans and events.
    `custom_instructions` are appended at the end and only help so much: "Yo me voy a dormir unos
    minutos" became the memory "User is going to sleep for a few minutes", which later made a fresh
    session go to sleep on its first, unclear chunk.
- **Judging each turn alone lacks context.** Whether something is a lasting fact or only true for
  the moment is often clear from the rest of the conversation, not from the single turn.

---

## Planned: hourly condenser

Replace per-turn extraction with a periodic consolidation over the whole conversation.

### Session log on disk

Every turn is appended, with its timestamp, to `~/.junior/session.jsonl` as it happens. This is
what makes the design safe against shutdowns: nothing lives only in the process.

- At startup, turns from the last hour are loaded back into the session, so a restart or deploy
  doesn't make Junior forget the conversation it was having.
- A half-written last line (power cut) is skipped.

### Condenser

Every hour, and at startup for anything left over:

1. Take the current facts and the session-log turns not condensed yet.
2. One LLM call (e.g. `gpt-4o-mini`): *"Here is what you know about the user and today's
   conversation. Return the updated list of lasting facts: merge duplicates, apply corrections
   (newer wins), drop anything only true for the moment. Never drop a fact unless it is
   contradicted or clearly temporary."*
3. Save the new list (backing up the previous one and logging the diff), then mark those turns as
   condensed.

A shutdown before the hour is up loses nothing: the turns are on disk and get condensed at the next
start.

### Storage: `facts.json` instead of mem0

With the condenser doing the extraction and no search in use, mem0 would only be storage. A plain
`~/.junior/facts.json` (read every turn, rewritten by the condenser) is simpler: one service less,
no Chroma, no embedding calls, and a file that can be read or edited by hand. The condenser can
start from the current mem0 facts.

### Why this is better

- The condenser sees the whole hour, so temporary things are recognisable as temporary.
- Duplicates and contradictions are cleaned on every run instead of accumulating.
- One LLM call per hour instead of one per turn.

### Risks

- The LLM drops a real fact → backup and diff on every run; instruction to drop only contradicted
  or temporary facts.
- Facts grow into the thousands → bring back search (embeddings or keyword), only then.

---

## Not doing (now)

- **Local embeddings / graph store** — no search is needed while all facts fit in the prompt.
- **Search per turn** — would need a transcript before the reply (+0.5–1 s per turn) or a second
  model round trip; not worth it with a few dozen facts.
- **A `recall` tool** — reintroduces the free-tool-call unreliability; if search is ever needed,
  a `recall` field inside `respond` is the safer shape.
