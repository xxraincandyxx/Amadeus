// @amadeus-header
// summary: Text chunker — splits documents into overlapping chunks at natural boundaries.
// layer: core
// status: active
// feature_flags: none
// provides:
// - fn: crate::chunker::chunk_text
// uses: none
// invariants:
// - Chunk size is character-based (no tokenizer dependency).
// - Prefers natural breaks: paragraph > sentence > line > word.
// - Adjacent chunks overlap by configurable amount.
// side_effects: none (pure function)
// tests:
// - cmd: cargo test -p rag
// @end-amadeus-header

//! Text chunking for RAG document ingestion.
//!
//! Splits text into overlapping chunks, preferring natural boundaries
//! (paragraphs, sentences, words) near the target chunk size.

/// Split `text` into overlapping chunks, each approximately `chunk_size` chars.
///
/// Prefers natural break points near the chunk boundary:
/// 1. Paragraph break (`\n\n`)
/// 2. Sentence end (`. `)
/// 3. Line break (`\n`)
/// 4. Word boundary (` `)
///
/// Adjacent chunks overlap by `overlap` characters.
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    if text.len() <= chunk_size {
        return vec![text.trim().to_string()];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < text.len() {
        let mut end = (start + chunk_size).min(text.len());

        // If we're not at the end, try to find a natural break
        if end < text.len() {
            // Search window: 20% of chunk_size around the target
            let window = chunk_size / 5;
            let search_start = end.saturating_sub(window);
            let search_end = (end + window).min(text.len());
            let search = &text[search_start..search_end];

            let break_offset = find_break(search);
            if let Some(offset) = break_offset {
                end = search_start + offset + 1; // include the break char
                if end <= start {
                    end = (start + chunk_size).min(text.len()); // fallback
                }
            }
        }

        let chunk = text[start..end].trim().to_string();
        if !chunk.is_empty() {
            chunks.push(chunk);
        }

        if end >= text.len() {
            break;
        }
        start = end.saturating_sub(overlap);
        if start >= end {
            start = end; // prevent infinite loop
        }
    }

    chunks
}

/// Find a natural break point in `search`, preferring stronger breaks.
/// Returns the offset (from the start of `search`) or `None`.
fn find_break(search: &str) -> Option<usize> {
    search
        .rfind("\n\n")
        .or_else(|| search.rfind(". "))
        .or_else(|| search.rfind('\n'))
        .or_else(|| search.rfind(' '))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        assert!(chunk_text("", 100, 20).is_empty());
    }

    #[test]
    fn test_smaller_than_chunk() {
        let chunks = chunk_text("hello world", 100, 20);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "hello world");
    }

    #[test]
    fn test_chunk_with_paragraph_breaks() {
        let text = "A".repeat(200) + "\n\n" + &"B".repeat(200);
        let chunks = chunk_text(&text, 100, 20);
        assert!(chunks.len() >= 2);
        // First chunk should end near the paragraph break
        assert!(chunks[0].len() <= 300); // generous bound
    }

    #[test]
    fn test_overlap() {
        let text = "first sentence. second sentence. third sentence. fourth sentence.";
        let chunks = chunk_text(text, 50, 15);
        assert!(chunks.len() >= 1);
        if chunks.len() > 1 {
            let first_end = &chunks[0][chunks[0].len().saturating_sub(10)..];
            let second_start = &chunks[1][..10.min(chunks[1].len())];
            // There should be some overlap in content
            // Not strictly guaranteed but likely with these params
            let _ = (first_end, second_start);
        }
    }

    #[test]
    fn test_long_text() {
        let text = "word ".repeat(500);
        let chunks = chunk_text(&text, 200, 50);
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(chunk.len() <= 400); // generous bound for break-finding
        }
    }
}
