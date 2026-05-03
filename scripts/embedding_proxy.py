#!/usr/bin/env python3
"""Lightweight OpenAI-compatible /v1/embeddings proxy using fastembed.

Usage:
    python embedding_proxy.py --port 1113

Then configure amadeus with:
    embedding_base_url: "http://localhost:1113/v1"
"""

from __future__ import annotations

import argparse
import asyncio
import json
from http.server import BaseHTTPRequestHandler, HTTPServer
from typing import Any

import numpy as np  # pyright: ignore[reportMissingImports]
from fastembed import TextEmbedding  # pyright: ignore[reportMissingImports]


class EmbeddingHandler(BaseHTTPRequestHandler):
    model: TextEmbedding | None = None

    def do_GET(self) -> None:
        if self.path == "/health":
            self._json(200, {"status": "ok"})
        else:
            self._json(404, {"error": "not found"})

    def do_POST(self) -> None:
        if self.path != "/v1/embeddings":
            self._json(404, {"error": "not found"})
            return

        content_length = int(self.headers.get("Content-Length", 0))
        body = json.loads(self.rfile.read(content_length))
        inputs = body.get("input", [])
        if isinstance(inputs, str):
            inputs = [inputs]

        if not inputs:
            self._json(400, {"error": "input is required"})
            return

        model_name = self._model_name()
        embeddings = list(self.model.embed(inputs))

        data = [
            {"index": i, "embedding": emb.tolist(), "object": "embedding"}
            for i, emb in enumerate(embeddings)
        ]

        self._json(200, {
            "object": "list",
            "data": data,
            "model": model_name,
            "usage": {"prompt_tokens": sum(len(t.split()) for t in inputs), "total_tokens": 0},
        })

    def _model_name(self) -> str:
        return getattr(self.model, "model_name", "fastembed")

    def _json(self, status: int, data: dict[str, Any]) -> None:
        body = json.dumps(data).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, fmt: str, *args: Any) -> None:
        print(f"[embedding_proxy] {args[0]}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=1113)
    parser.add_argument("--model", type=str, default="BAAI/bge-small-en-v1.5")
    args = parser.parse_args()

    print(f"Loading embedding model {args.model}...")
    EmbeddingHandler.model = TextEmbedding(model_name=args.model)
    dim = len(list(EmbeddingHandler.model.embed(["test"]))[0])
    print(f"Model loaded, dim={dim}")

    server = HTTPServer(("0.0.0.0", args.port), EmbeddingHandler)
    print(f"Embedding proxy listening on http://0.0.0.0:{args.port}/v1/embeddings")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down.")
        server.shutdown()


if __name__ == "__main__":
    main()
