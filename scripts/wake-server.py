#!/usr/bin/env python3
"""Local speech-to-text for Junior's wake-up phrase, so nothing leaves the robot while it sleeps.

  POST /transcribe  — body: a WAV file; returns {"text": "..."}

Uses faster-whisper `tiny` on the CPU, with Spanish fixed and a prompt that primes it for
"Junior, despierta". Measured on the Pi 5 with real recordings: ~1.1 s per phrase, every
wake phrase transcribed correctly (English "Junior, wake up" too), and empty text for noise.
The Rust side decides whether the text is the wake-up phrase.
"""

import io
import logging
import os

import uvicorn
from fastapi import FastAPI, Request
from faster_whisper import WhisperModel

logging.basicConfig(level=logging.INFO, format="%(asctime)s [wake] %(message)s")
logger = logging.getLogger(__name__)

MODEL = os.environ.get("WAKE_MODEL", "tiny")
LANGUAGE = os.environ.get("WAKE_LANGUAGE", "es")
PROMPT = os.environ.get("WAKE_PROMPT", "Junior, despierta.")

model = WhisperModel(MODEL, device="cpu", compute_type="int8", cpu_threads=4)
app = FastAPI()


@app.post("/transcribe")
async def transcribe(request: Request):
    audio = io.BytesIO(await request.body())
    segments, _ = model.transcribe(
        audio, beam_size=1, vad_filter=False, language=LANGUAGE, initial_prompt=PROMPT
    )
    text = " ".join(s.text.strip() for s in segments).strip()
    # Only the length is logged: what is said near a sleeping Junior stays private.
    logger.info("transcribed %d chars", len(text))
    return {"text": text}


if __name__ == "__main__":
    uvicorn.run(app, host="127.0.0.1", port=8001, log_level="warning")
