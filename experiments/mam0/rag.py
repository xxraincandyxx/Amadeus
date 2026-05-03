"""BM25 RAG store for mam0 agent.

Zero-dependency keyword retrieval with JSON persistence.
Matches Amadeus Rust RAG patterns (chunker, JSON persistence, document API)
but uses BM25 instead of embeddings since the vLLM endpoint lacks /v1/embeddings.
"""

from __future__ import annotations

import json
import math
import re
import uuid
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

DEFAULT_CHUNK_SIZE = 1200
DEFAULT_CHUNK_OVERLAP = 200
DEFAULT_TOP_K = 5
BM25_K1 = 1.5
BM25_B = 0.75

# ---------------------------------------------------------------------------
# Tokenizer
# ---------------------------------------------------------------------------

_WORD_RE = re.compile(r"[^a-zA-Z0-9]+")


def tokenize(text: str) -> list[str]:
    """Lowercase, split on non-alphanumeric runs, drop tokens <= 1 char."""
    return [t for t in _WORD_RE.split(text.lower()) if len(t) > 1]


# ---------------------------------------------------------------------------
# Chunker (ported from crates/rag/src/chunker.rs)
# ---------------------------------------------------------------------------


def chunk_text(
    text: str,
    chunk_size: int = DEFAULT_CHUNK_SIZE,
    overlap: int = DEFAULT_CHUNK_OVERLAP,
) -> list[str]:
    """Split text into overlapping chunks, preferring natural breakpoints."""
    text = text.strip()
    if not text:
        return []
    if len(text) <= chunk_size:
        return [text]

    chunks: list[str] = []
    pos = 0
    while pos < len(text):
        end = pos + chunk_size
        if end >= len(text):
            chunks.append(text[pos:].strip())
            break

        # Search within a 20% window backward from target end for a natural break
        window = max(0, int(chunk_size * 0.2))
        search_start = end - window
        chunk_slice = text[search_start:end]

        # Preferred breakpoints: paragraph > sentence > line > word
        best = -1
        for sep in ("\n\n", ". ", "\n", " "):
            idx = chunk_slice.rfind(sep)
            if idx != -1:
                best = idx + len(sep)
                break

        if best != -1:
            split_point = search_start + best
        else:
            split_point = end

        chunk = text[pos:split_point].strip()
        if chunk:
            chunks.append(chunk)

        # Advance with overlap
        pos = max(pos + 1, split_point - overlap)

    return chunks


# ---------------------------------------------------------------------------
# BM25 scoring
# ---------------------------------------------------------------------------


def _compute_idf(term: str, doc_freqs: dict[str, int], total_chunks: int) -> float:
    n = doc_freqs.get(term, 0)
    return math.log((total_chunks - n + 0.5) / (n + 0.5) + 1.0)


def _bm25_score(
    query_terms: list[str],
    chunk_tf: dict[str, int],
    chunk_len: int,
    avg_chunk_len: float,
    doc_freqs: dict[str, int],
    total_chunks: int,
) -> float:
    if total_chunks == 0 or avg_chunk_len == 0:
        return 0.0

    score = 0.0
    for term in query_terms:
        tf = chunk_tf.get(term, 0)
        if tf == 0:
            continue
        idf = _compute_idf(term, doc_freqs, total_chunks)
        numerator = tf * (BM25_K1 + 1)
        denominator = tf + BM25_K1 * (1 - BM25_B + BM25_B * (chunk_len / avg_chunk_len))
        score += idf * numerator / denominator
    return score


# ---------------------------------------------------------------------------
# RAGStore
# ---------------------------------------------------------------------------


class RAGStore:
    """BM25 keyword search over chunked documents, persisted as JSON."""

    def __init__(self, index_path: Path | None = None):
        self.index_path = Path(index_path) if index_path else Path.cwd() / "rag_index.json"
        self.documents: dict[str, dict] = {}
        self.doc_freqs: dict[str, int] = {}
        self.total_chunks: int = 0
        self.avg_chunk_length: float = 0.0
        self._load()

    # ---- persistence -------------------------------------------------------

    def _load(self) -> None:
        if not self.index_path.exists():
            return
        try:
            raw = self.index_path.read_text()
            if not raw.strip():
                return
            data = json.loads(raw)
        except (json.JSONDecodeError, OSError):
            print(f"[rag] Warning: could not parse {self.index_path}, starting fresh")
            return

        self.documents = data.get("documents", {})
        if "doc_freqs" in data:
            self.doc_freqs = data["doc_freqs"]
            self.total_chunks = data.get("total_chunks", 0)
            self.avg_chunk_length = data.get("avg_chunk_length", 0.0)
        else:
            # Old-format file — rebuild stats
            self._rebuild_stats()

    def _save(self) -> None:
        self.index_path.parent.mkdir(parents=True, exist_ok=True)
        payload = {
            "documents": self.documents,
            "doc_freqs": self.doc_freqs,
            "total_chunks": self.total_chunks,
            "avg_chunk_length": self.avg_chunk_length,
        }
        self.index_path.write_text(json.dumps(payload, indent=2, ensure_ascii=False))

    def _rebuild_stats(self) -> None:
        self.doc_freqs.clear()
        self.total_chunks = 0
        total_terms = 0
        for doc in self.documents.values():
            for chunk in doc.get("chunks", []):
                self.total_chunks += 1
                tf = chunk.get("term_freqs", {})
                for term in tf:
                    self.doc_freqs[term] = self.doc_freqs.get(term, 0) + 1
                total_terms += sum(tf.values())
        if self.total_chunks > 0:
            self.avg_chunk_length = total_terms / self.total_chunks

    # ---- ingest ------------------------------------------------------------

    def ingest(
        self,
        text: str = "",
        path: str = "",
        document_id: str | None = None,
        chunk_size: int = DEFAULT_CHUNK_SIZE,
        chunk_overlap: int = DEFAULT_CHUNK_OVERLAP,
    ) -> str:
        if path:
            try:
                text = Path(path).read_text()
            except Exception as e:
                return f"Error reading {path}: {e}"
        if not text:
            return "Error: one of 'path' or 'text' is required."

        doc_id = document_id or uuid.uuid4().hex[:12]

        chunks = chunk_text(text, chunk_size, chunk_overlap)
        if not chunks:
            return f"No content to ingest for document '{doc_id}'."

        # Remove old document if re-ingesting
        if doc_id in self.documents:
            self.documents.pop(doc_id)

        ingest_time = datetime.now(timezone.utc).isoformat()
        chunk_entries: list[dict] = []
        for i, chunk_content in enumerate(chunks):
            tf = dict(Counter(tokenize(chunk_content)))
            chunk_entries.append({
                "index": i,
                "content": chunk_content,
                "term_freqs": tf,
            })

        self.documents[doc_id] = {
            "id": doc_id,
            "original_path": path or None,
            "ingested_at": ingest_time,
            "chunks": chunk_entries,
        }

        self._rebuild_stats()
        self._save()
        return (
            f"Ingested {len(chunks)} chunk(s) for document '{doc_id}' "
            f"({self.total_chunks} total chunks across {len(self.documents)} document(s))."
        )

    # ---- query -------------------------------------------------------------

    def query(self, query_text: str = "", top_k: int = DEFAULT_TOP_K) -> str:
        if not query_text:
            return "Error: 'query_text' is required."
        if self.total_chunks == 0:
            return "No documents ingested yet. Use rag_ingest first."

        query_terms = tokenize(query_text)
        if not query_terms:
            return "Error: query contains no indexable terms."

        scored: list[tuple[float, str, dict]] = []
        for doc_id, doc in self.documents.items():
            for chunk in doc.get("chunks", []):
                tf = chunk.get("term_freqs", {})
                chunk_len = sum(tf.values())
                s = _bm25_score(
                    query_terms, tf, chunk_len,
                    self.avg_chunk_length, self.doc_freqs, self.total_chunks,
                )
                if s > 0:
                    scored.append((s, doc_id, chunk))

        scored.sort(key=lambda x: x[0], reverse=True)
        top = scored[:top_k]

        if not top:
            return f'No relevant chunks found for "{query_text}".'

        lines = [f'Found {len(top)} result(s) for "{query_text}":', ""]
        for rank, (score, doc_id, chunk) in enumerate(top, 1):
            content = chunk["content"]
            truncated = content[:500]
            if len(content) > 500:
                truncated += "..."
            lines.append(
                f"{rank}. [score:{score:.3f}] doc:{doc_id} chunk:{chunk['index']}\n"
                f"   {truncated}\n"
            )
        return "\n".join(lines)

    # ---- list / delete -----------------------------------------------------

    def list_documents(self) -> str:
        if not self.documents:
            return "No documents ingested."

        lines = [f"{len(self.documents)} document(s):"]
        for doc_id, doc in self.documents.items():
            chunks_n = len(doc.get("chunks", []))
            ingested = doc.get("ingested_at", "unknown")
            src = doc.get("original_path") or ""
            extra = f", from {src}" if src else ""
            lines.append(f"  - {doc_id} ({chunks_n} chunks, ingested {ingested}{extra})")
        return "\n".join(lines)

    def delete_document(self, document_id: str = "") -> str:
        if not document_id:
            return "Error: 'document_id' is required."
        if document_id not in self.documents:
            return f"Document '{document_id}' not found."

        n = len(self.documents[document_id].get("chunks", []))
        del self.documents[document_id]
        self._rebuild_stats()
        self._save()
        return (
            f"Deleted {n} chunk(s) for document '{document_id}' "
            f"({self.total_chunks} total chunks across {len(self.documents)} document(s))."
        )
