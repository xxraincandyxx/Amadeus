// @amadeus-header
// summary: LocalHashEmbedder — zero-dependency on-device embedding via feature hashing.
// layer: core
// status: active
// feature_flags: none
// provides:
// - type: crate::local_embedding::LocalHashEmbedder
// - const: crate::local_embedding::DEFAULT_DIMENSION
// uses:
// - trait: crate::embedding::Embedder
// invariants:
// - Deterministic: identical input text always yields the identical vector.
// - Output vectors are L2-normalized (zero vector for tokenless input).
// - No network, no model file, no external dependencies.
// side_effects: none
// tests:
// - cmd: cargo test -p rag
// @end-amadeus-header

//! On-device embedding baseline: feature hashing ("hashing trick").
//!
//! Each text is tokenized (Latin words plus CJK bigrams), every token is
//! FNV-1a hashed into a fixed-dimension accumulator with a hash-derived
//! sign, and the result is L2-normalized. The vectors capture lexical
//! overlap only — not deep semantics — but they are deterministic, run in
//! microseconds on any CPU (including domestic edge hardware), and need no
//! model file, making this the always-available fallback backend for edge
//! deployments. Semantic-quality backends (remote model, Kylin SDK) plug
//! into the same [`Embedder`](crate::embedding::Embedder) contract.

use crate::embedding::{Embedder, EmbeddingError};

/// Default output dimension for [`LocalHashEmbedder`].
pub const DEFAULT_DIMENSION: usize = 384;

/// Zero-dependency, on-device embedding via feature hashing.
#[derive(Debug, Clone)]
pub struct LocalHashEmbedder {
    dimension: usize,
}

impl LocalHashEmbedder {
    /// Create an embedder producing `dimension`-dimensional vectors.
    ///
    /// A dimension of 0 is clamped to [`DEFAULT_DIMENSION`].
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension: if dimension == 0 {
                DEFAULT_DIMENSION
            } else {
                dimension
            },
        }
    }

    fn embed_one(&self, text: &str) -> Vec<f32> {
        let mut acc = vec![0.0f32; self.dimension];
        for token in tokenize(text) {
            let h = fnv1a_64(token.as_bytes());
            let index = (h % self.dimension as u64) as usize;
            // Second, decorrelated bit of the hash picks the sign so that
            // colliding tokens still cancel randomly instead of systematically.
            let sign = if (h >> 17) & 1 == 0 { 1.0 } else { -1.0 };
            acc[index] += sign;
        }
        l2_normalize(acc)
    }
}

#[async_trait::async_trait]
impl Embedder for LocalHashEmbedder {
    fn name(&self) -> &'static str {
        "local_hash"
    }

    fn dimension(&self) -> Option<usize> {
        Some(self.dimension)
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts.iter().map(|t| self.embed_one(t)).collect())
    }
}

/// Split into lowercase Latin/digit word tokens and CJK character bigrams.
///
/// CJK text has no whitespace separators, so a run of n CJK characters
/// yields its bigrams (a single isolated CJK char yields itself as a
/// unigram). Everything else (punctuation, whitespace) is a separator.
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    let mut cjk_run: Vec<char> = Vec::new();

    let flush_word = |word: &mut String, tokens: &mut Vec<String>| {
        if !word.is_empty() {
            tokens.push(std::mem::take(word));
        }
    };
    let flush_cjk = |run: &mut Vec<char>, tokens: &mut Vec<String>| {
        if run.len() == 1 {
            tokens.push(run[0].to_string());
        } else {
            for pair in run.windows(2) {
                tokens.push(pair.iter().collect());
            }
        }
        run.clear();
    };

    for ch in text.chars() {
        if is_cjk(ch) {
            flush_word(&mut word, &mut tokens);
            cjk_run.push(ch);
        } else if ch.is_alphanumeric() {
            flush_cjk(&mut cjk_run, &mut tokens);
            word.extend(ch.to_lowercase());
        } else {
            flush_word(&mut word, &mut tokens);
            flush_cjk(&mut cjk_run, &mut tokens);
        }
    }
    flush_word(&mut word, &mut tokens);
    flush_cjk(&mut cjk_run, &mut tokens);
    tokens
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x4E00..=0x9FFF  // CJK Unified Ideographs
        | 0x3400..=0x4DBF  // CJK Extension A
        | 0x3000..=0x303F  // CJK symbols and punctuation
        | 0xFF00..=0xFFEF // fullwidth forms
    )
}

/// FNV-1a 64-bit: tiny, deterministic, good-enough avalanche for hashing.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}

fn l2_normalize(mut v: Vec<f32>) -> Vec<f32> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_deterministic_and_normalized() {
        let embedder = LocalHashEmbedder::new(128);
        let a = embedder.embed_single("配置文件加载失败").await.unwrap();
        let b = embedder.embed_single("配置文件加载失败").await.unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 128);
        let norm = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[tokio::test]
    async fn test_dimension_configurable_and_clamped() {
        let e = LocalHashEmbedder::new(64);
        assert_eq!(e.dimension(), Some(64));
        let v = e.embed_single("hello world").await.unwrap();
        assert_eq!(v.len(), 64);

        let clamped = LocalHashEmbedder::new(0);
        assert_eq!(clamped.dimension(), Some(DEFAULT_DIMENSION));
    }

    #[tokio::test]
    async fn test_lexical_overlap_scores_higher_than_unrelated() {
        let embedder = LocalHashEmbedder::new(DEFAULT_DIMENSION);
        let query = embedder.embed_single("内存泄漏排查").await.unwrap();
        let related = embedder.embed_single("排查内存泄漏的方法").await.unwrap();
        let unrelated = embedder.embed_single("今天天气很好").await.unwrap();

        let sim_related = crate::vector_store::cosine_similarity(&query, &related);
        let sim_unrelated = crate::vector_store::cosine_similarity(&query, &unrelated);
        assert!(
            sim_related > sim_unrelated,
            "related={} should exceed unrelated={}",
            sim_related,
            sim_unrelated
        );
        assert!(sim_related > 0.0);
    }

    #[tokio::test]
    async fn test_mixed_cjk_and_latin() {
        let embedder = LocalHashEmbedder::new(DEFAULT_DIMENSION);
        let a = embedder
            .embed_single("使用 rust 编写 embedder")
            .await
            .unwrap();
        let b = embedder.embed_single("rust embedder 用法").await.unwrap();
        let sim = crate::vector_store::cosine_similarity(&a, &b);
        assert!(sim > 0.0, "shared tokens should yield positive similarity");
    }

    #[tokio::test]
    async fn test_empty_text_yields_zero_vector() {
        let embedder = LocalHashEmbedder::new(32);
        let v = embedder.embed_single("  !…  ").await.unwrap();
        assert!(v.iter().all(|x| x.abs() < f32::EPSILON));
    }

    #[test]
    fn test_tokenize_cjk_bigrams() {
        let tokens = tokenize("内存泄漏");
        assert_eq!(tokens, vec!["内存", "存泄", "泄漏"]);
        let single = tokenize("火");
        assert_eq!(single, vec!["火"]);
    }

    #[test]
    fn test_tokenize_latin_words_lowercased() {
        let tokens = tokenize("Hello, World!");
        assert_eq!(tokens, vec!["hello", "world"]);
    }
}
