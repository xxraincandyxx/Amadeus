"""RAG management — ingest, query, list, and delete documents via the API."""

from __future__ import annotations

from typing import Optional

from .client import AmadeusClient
from .types import RagDocumentInfo, RagIngestResponse, RagQueryResponse, RagSearchResult


class RAGManager:
    """Manage RAG document ingestion and semantic search via the API.

    Usage::

        rag = RAGManager(client)
        await rag.ingest_text("Amadeus is a Rust AI agent SDK.", doc_id="about")
        results = await rag.query("What language is Amadeus written in?")
        docs = await rag.list_documents()
        await rag.delete_document("about")
    """

    def __init__(self, client: AmadeusClient) -> None:
        self._client = client

    async def ingest_text(
        self,
        text: str,
        document_id: Optional[str] = None,
        chunk_size: Optional[int] = None,
        chunk_overlap: Optional[int] = None,
    ) -> RagIngestResponse:
        """Ingest raw text into the vector store.

        Args:
            text: The text content to chunk and embed.
            document_id: Optional human-readable ID. Auto-generated if omitted.
            chunk_size: Target characters per chunk.
            chunk_overlap: Overlap characters between adjacent chunks.

        Returns:
            RagIngestResponse with document_id and chunk_count.
        """
        return await self._client.rag_ingest(
            text=text,
            document_id=document_id,
            chunk_size=chunk_size,
            chunk_overlap=chunk_overlap,
        )

    async def ingest_file(
        self,
        path: str,
        document_id: Optional[str] = None,
        chunk_size: Optional[int] = None,
        chunk_overlap: Optional[int] = None,
    ) -> RagIngestResponse:
        """Ingest a local file into the vector store.

        Args:
            path: Path to the file to ingest.
            document_id: Optional human-readable ID. Auto-generated if omitted.
            chunk_size: Target characters per chunk.
            chunk_overlap: Overlap characters between adjacent chunks.

        Returns:
            RagIngestResponse with document_id and chunk_count.
        """
        return await self._client.rag_ingest(
            path=path,
            document_id=document_id,
            chunk_size=chunk_size,
            chunk_overlap=chunk_overlap,
        )

    async def query(self, query: str, top_k: int = 5) -> list[RagSearchResult]:
        """Semantic search over ingested documents.

        Args:
            query: Natural language search query.
            top_k: Number of top results to return.

        Returns:
            List of RagSearchResult with content and relevance scores.
        """
        resp = await self._client.rag_query(query, top_k=top_k)
        return resp.results

    async def list_documents(self) -> list[RagDocumentInfo]:
        """List all ingested documents with chunk counts.

        Returns:
            List of RagDocumentInfo.
        """
        resp = await self._client.rag_list_documents()
        return resp.documents

    async def delete_document(self, document_id: str) -> dict:
        """Delete a document and all its chunks.

        Args:
            document_id: The document ID to delete.

        Returns:
            API response dict with deleted count and document_id.
        """
        return await self._client.rag_delete_document(document_id)
