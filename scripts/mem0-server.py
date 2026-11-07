#!/usr/bin/env python3
"""Self-hosted mem0 memory server for Junior.

Exposes a minimal REST API that the Rust client calls:
  POST /v1/memories/        — extract and store facts from a conversation turn
  POST /v1/memories/search/ — retrieve relevant memories by semantic query
  GET  /v1/memories/        — list all stored memories (for inspection)
"""

import logging
import os

import uvicorn
from fastapi import FastAPI
from mem0 import Memory
from pydantic import BaseModel

logging.basicConfig(level=logging.INFO, format="%(asctime)s [mem0] %(message)s")
logger = logging.getLogger(__name__)

config = {
    "llm": {
        "provider": "openai",
        "config": {
            "model": "gpt-4o-mini",
            "api_key": os.environ["OPENAI_API_KEY"],
        },
    },
    "embedder": {
        "provider": "openai",
        "config": {
            "model": "text-embedding-3-small",
            "api_key": os.environ["OPENAI_API_KEY"],
        },
    },
    "vector_store": {
        "provider": "chroma",
        "config": {
            "collection_name": "junior",
            "path": os.path.expanduser("~/.junior-mem0"),
        },
    },
    # Long-term memory only; the robot keeps the last hour of conversation itself.
    "custom_instructions": (
        "This is the long-term memory of Junior, a small robot pet. Only keep lasting facts "
        "about the user, the people and pets in their life, their home, routines and "
        "preferences. Do not keep greetings, small talk, questions to the robot, requests or "
        "commands to the robot (moving, being quiet, talking again), what the robot said, or "
        "unclear, noisy or misheard audio. If a fact corrects an earlier one, state the "
        "corrected fact on its own."
    ),
}

memory = Memory.from_config(config)
app = FastAPI()


class AddRequest(BaseModel):
    messages: list
    user_id: str


class SearchRequest(BaseModel):
    query: str
    user_id: str
    limit: int = 5


def _results_list(raw) -> list:
    """Normalise mem0 response to a plain list regardless of version."""
    if isinstance(raw, dict):
        return raw.get("results", [])
    return raw or []


@app.post("/v1/memories/")
def add_memory(req: AddRequest):
    result = memory.add(req.messages, user_id=req.user_id)
    extracted = _results_list(result)
    logger.info("add user=%s extracted=%d facts", req.user_id, len(extracted))
    return result


@app.post("/v1/memories/search/")
def search_memory(req: SearchRequest):
    raw = memory.search(req.query, user_id=req.user_id, limit=req.limit)
    results = _results_list(raw)
    logger.info("search user=%s query=%r hits=%d", req.user_id, req.query, len(results))
    return {"results": results}


@app.get("/v1/memories/")
def get_all(user_id: str = "junior"):
    # mem0 returns only 20 by default; the caller picks the newest from the full list.
    raw = memory.get_all(filters={"user_id": user_id}, top_k=1000)
    return {"results": _results_list(raw)}


if __name__ == "__main__":
    uvicorn.run(app, host="127.0.0.1", port=8000, log_level="warning")
