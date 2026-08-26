// @amadeus-header
// summary: Deterministic Chinese-oriented sensitive-data detection and redaction.
// layer: core
// status: active
// feature_flags: none
// provides:
// - type: crate::SensitiveDataDetector
// - type: crate::SensitiveKind
// - type: crate::SensitiveSpan
// uses:
// - crate: regex
// invariants:
// - Detection offsets are UTF-8 byte offsets suitable for safe reverse-order replacement.
// - Secret and credential values never appear in redacted output.
// side_effects: none
// tests:
// - cmd: cargo test -p privacy
// @end-amadeus-header

//! Sensitive-data detection and redaction for memory ingestion boundaries.

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitiveKind {
    Email,
    PhoneNumber,
    NationalId,
    BankCard,
    ApiKey,
    Password,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensitiveSpan {
    pub kind: SensitiveKind,
    pub start: usize,
    pub end: usize,
    pub confidence: f32,
    pub risk: RiskLevel,
    pub recognizer: String,
}

#[derive(Debug)]
pub struct SensitiveDataDetector {
    email: Regex,
    phone: Regex,
    national_id: Regex,
    bank_card: Regex,
    api_key: Regex,
    password: Regex,
}

impl Default for SensitiveDataDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl SensitiveDataDetector {
    pub fn new() -> Self {
        Self {
            email: Regex::new(r"(?i)[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}")
                .expect("valid email regex"),
            phone: Regex::new(r"(?:\+?86[- ]?)?1[3-9]\d{9}").expect("valid phone regex"),
            national_id: Regex::new(r"\d{17}[0-9Xx]").expect("valid national id regex"),
            bank_card: Regex::new(r"(?:\d[ -]?){15,18}\d").expect("valid bank card regex"),
            api_key: Regex::new(r"(?i)(?:sk|ak|api)[-_][A-Za-z0-9_-]{12,}")
                .expect("valid api key regex"),
            password: Regex::new(r"(?i)(?:密码|口令|password|passwd)\s*[:：=]\s*([^\s,，;；]{4,})")
                .expect("valid password regex"),
        }
    }

    pub fn detect(&self, text: &str) -> Vec<SensitiveSpan> {
        let mut spans = Vec::new();
        Self::extend_matches(
            &mut spans,
            &self.email,
            text,
            SensitiveKind::Email,
            RiskLevel::High,
            0.98,
            false,
        );
        Self::extend_matches(
            &mut spans,
            &self.phone,
            text,
            SensitiveKind::PhoneNumber,
            RiskLevel::High,
            0.99,
            false,
        );
        for found in self.national_id.find_iter(text) {
            if bounded_by_non_digits(text, found.start(), found.end())
                && valid_chinese_national_id(found.as_str())
            {
                spans.push(Self::span(
                    found.start(),
                    found.end(),
                    SensitiveKind::NationalId,
                    RiskLevel::Critical,
                    1.0,
                    "regex+checksum",
                ));
            }
        }
        for found in self.bank_card.find_iter(text) {
            let digits: String = found
                .as_str()
                .chars()
                .filter(char::is_ascii_digit)
                .collect();
            if bounded_by_non_digits(text, found.start(), found.end()) && luhn_valid(&digits) {
                spans.push(Self::span(
                    found.start(),
                    found.end(),
                    SensitiveKind::BankCard,
                    RiskLevel::Critical,
                    1.0,
                    "regex+luhn",
                ));
            }
        }
        Self::extend_matches(
            &mut spans,
            &self.api_key,
            text,
            SensitiveKind::ApiKey,
            RiskLevel::Critical,
            0.99,
            false,
        );
        Self::extend_matches(
            &mut spans,
            &self.password,
            text,
            SensitiveKind::Password,
            RiskLevel::Critical,
            0.99,
            true,
        );
        let critical_ranges: Vec<(usize, usize)> = spans
            .iter()
            .filter(|span| span.risk == RiskLevel::Critical)
            .map(|span| (span.start, span.end))
            .collect();
        spans.retain(|span| {
            span.kind != SensitiveKind::PhoneNumber
                || !critical_ranges
                    .iter()
                    .any(|(start, end)| span.start >= *start && span.end <= *end)
        });
        spans.sort_by_key(|span| (span.start, span.end));
        spans.dedup_by(|left, right| left.start == right.start && left.end == right.end);
        spans
    }

    pub fn redact(&self, text: &str) -> (String, Vec<SensitiveSpan>) {
        let spans = self.detect(text);
        let mut redacted = text.to_string();
        for span in spans.iter().rev() {
            let replacement = format!("[REDACTED:{:?}]", span.kind).to_uppercase();
            redacted.replace_range(span.start..span.end, &replacement);
        }
        (redacted, spans)
    }

    pub fn contains_critical(&self, text: &str) -> bool {
        self.detect(text)
            .iter()
            .any(|span| span.risk == RiskLevel::Critical)
    }

    fn extend_matches(
        spans: &mut Vec<SensitiveSpan>,
        regex: &Regex,
        text: &str,
        kind: SensitiveKind,
        risk: RiskLevel,
        confidence: f32,
        capture_one: bool,
    ) {
        for captures in regex.captures_iter(text) {
            let found = if capture_one {
                captures.get(1)
            } else {
                captures.get(0)
            };
            if let Some(found) = found {
                spans.push(Self::span(
                    found.start(),
                    found.end(),
                    kind,
                    risk,
                    confidence,
                    "regex",
                ));
            }
        }
    }

    fn span(
        start: usize,
        end: usize,
        kind: SensitiveKind,
        risk: RiskLevel,
        confidence: f32,
        recognizer: &str,
    ) -> SensitiveSpan {
        SensitiveSpan {
            kind,
            start,
            end,
            confidence,
            risk,
            recognizer: recognizer.to_string(),
        }
    }
}

fn valid_chinese_national_id(value: &str) -> bool {
    if value.len() != 18 || !value.as_bytes()[..17].iter().all(u8::is_ascii_digit) {
        return false;
    }
    let weights = [7u32, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2];
    let checks = b"10X98765432";
    let sum: u32 = value.as_bytes()[..17]
        .iter()
        .zip(weights)
        .map(|(digit, weight)| u32::from(digit - b'0') * weight)
        .sum();
    value.as_bytes()[17].to_ascii_uppercase() == checks[(sum % 11) as usize]
}

fn luhn_valid(value: &str) -> bool {
    if !(16..=19).contains(&value.len()) || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let mut sum = 0u32;
    let parity = value.len() % 2;
    for (index, byte) in value.bytes().enumerate() {
        let mut digit = u32::from(byte - b'0');
        if index % 2 == parity {
            digit *= 2;
            if digit > 9 {
                digit -= 9;
            }
        }
        sum += digit;
    }
    sum.is_multiple_of(10)
}

fn bounded_by_non_digits(value: &str, start: usize, end: usize) -> bool {
    let left_ok = value[..start]
        .chars()
        .next_back()
        .map(|ch| !ch.is_ascii_digit())
        .unwrap_or(true);
    let right_ok = value[end..]
        .chars()
        .next()
        .map(|ch| !ch.is_ascii_digit())
        .unwrap_or(true);
    left_ok && right_ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_and_redacts_chinese_sensitive_values() {
        let detector = SensitiveDataDetector::new();
        let input = "电话13800138000，邮箱test@example.com，身份证11010519491231002X，银行卡4532015112830366";
        let (redacted, spans) = detector.redact(input);
        assert_eq!(spans.len(), 4, "{spans:?}");
        assert!(!redacted.contains("13800138000"));
        assert!(!redacted.contains("test@example.com"));
        assert!(!redacted.contains("11010519491231002X"));
        assert!(!redacted.contains("4532015112830366"));
    }

    #[test]
    fn rejects_invalid_checksums_and_ordinary_numbers() {
        let detector = SensitiveDataDetector::new();
        let spans = detector.detect("订单1234567890123456，身份证110105194912310021");
        assert!(spans
            .iter()
            .all(|span| span.kind != SensitiveKind::NationalId));
        assert!(spans
            .iter()
            .all(|span| span.kind != SensitiveKind::BankCard));
    }

    #[test]
    fn detects_password_value_without_redacting_label() {
        let detector = SensitiveDataDetector::new();
        let (redacted, spans) = detector.redact("数据库密码：S3cret-value");
        assert_eq!(spans[0].kind, SensitiveKind::Password);
        assert!(redacted.starts_with("数据库密码："));
        assert!(!redacted.contains("S3cret-value"));
    }
}
