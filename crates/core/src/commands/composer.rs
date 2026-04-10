// @amadeus-header
// summary: Core composer helpers for citation mentions, rendered cite spans, and pasted path normalization.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate::commands::composer
// - type: crate::commands::composer::CitationCandidate
// - type: crate::commands::composer::ActiveCitationQuery
// - type: crate::commands::composer::CitationRenderSpan
// - fn: crate::commands::composer::scan_workspace_citation_candidates
// - fn: crate::commands::composer::find_active_citation_query
// - fn: crate::commands::composer::filter_citation_candidates
// - fn: crate::commands::composer::apply_citation_candidate
// - fn: crate::commands::composer::parse_render_spans
// - fn: crate::commands::composer::normalize_pasted_path
// - fn: crate::commands::composer::format_citation_markdown
// uses:
// - artifact: filesystem paths and files
// - format: Markdown links
// - protocol: URL parsing
// - runtime: std path and io utilities
// invariants:
// - Accepted composer citations serialize to markdown links targeting absolute paths.
// - Active citation query detection remains cursor-relative and deterministic.
// side_effects:
// - Reads filesystem state.
// tests:
// - cmd: cargo test -p core composer --features full
// @end-amadeus-header

use std::path::{Path, PathBuf};

use url::Url;
use walkdir::WalkDir;

const DEFAULT_SCAN_DEPTH: usize = 6;
const DEFAULT_MAX_RESULTS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationCandidate {
    pub label: String,
    pub absolute_path: String,
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveCitationQuery {
    pub row: usize,
    pub start_col: usize,
    pub end_col: usize,
    pub query: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationRenderSpan {
    pub line_index: usize,
    pub start_col: usize,
    pub end_col: usize,
    pub label: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationApplyResult {
    pub text: String,
    pub cursor: (usize, usize),
}

pub fn scan_workspace_citation_candidates(
    workdir: &Path,
) -> std::io::Result<Vec<CitationCandidate>> {
    let mut candidates = Vec::new();
    let canonical_workdir = workdir
        .canonicalize()
        .unwrap_or_else(|_| workdir.to_path_buf());

    for entry in WalkDir::new(workdir)
        .max_depth(DEFAULT_SCAN_DEPTH)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        let path = entry.path();
        if path == workdir || path == canonical_workdir {
            continue;
        }

        let file_type = entry.file_type();
        if !file_type.is_file() && !file_type.is_dir() {
            continue;
        }

        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name.starts_with('.')
            || path
                .components()
                .any(|component| component.as_os_str() == "target")
        {
            continue;
        }

        let absolute = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let relative = absolute
            .strip_prefix(&canonical_workdir)
            .or_else(|_| absolute.strip_prefix(workdir))
            .unwrap_or(&absolute);
        let label = absolute
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
            .unwrap_or_else(|| relative.to_string_lossy().to_string());

        candidates.push(CitationCandidate {
            label,
            absolute_path: absolute.display().to_string(),
            relative_path: relative.display().to_string(),
        });

        if candidates.len() >= DEFAULT_MAX_RESULTS {
            break;
        }
    }

    candidates.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(candidates)
}

pub fn find_active_citation_query(
    input: &str,
    cursor: (usize, usize),
) -> Option<ActiveCitationQuery> {
    let lines = split_lines_preserving_empty(input);
    let line = lines.get(cursor.0)?;
    let safe_col = cursor.1.min(line.chars().count());
    let char_vec = line.chars().collect::<Vec<_>>();

    let mut start = safe_col;
    while start > 0 && !char_vec[start - 1].is_whitespace() {
        start -= 1;
    }

    if char_vec.get(start).copied()? != '@' {
        return None;
    }

    if start > 0 {
        let prev = char_vec[start - 1];
        if prev.is_alphanumeric() || matches!(prev, ']' | ')' | '/' | '\\' | '.') {
            return None;
        }
    }

    let query = char_vec[start + 1..safe_col].iter().collect::<String>();
    if query.contains('\n') || query.contains('\r') || query.contains('[') || query.contains(']') {
        return None;
    }

    Some(ActiveCitationQuery {
        row: cursor.0,
        start_col: start,
        end_col: safe_col,
        query,
    })
}

pub fn filter_citation_candidates(
    candidates: &[CitationCandidate],
    query: &str,
    limit: usize,
) -> Vec<CitationCandidate> {
    let normalized = query.trim().to_ascii_lowercase();
    let mut filtered = candidates
        .iter()
        .filter(|candidate| {
            if normalized.is_empty() {
                true
            } else {
                candidate.label.to_ascii_lowercase().contains(&normalized)
                    || candidate
                        .relative_path
                        .to_ascii_lowercase()
                        .contains(&normalized)
            }
        })
        .cloned()
        .collect::<Vec<_>>();

    filtered.sort_by(|left, right| {
        let left_starts = left.label.to_ascii_lowercase().starts_with(&normalized);
        let right_starts = right.label.to_ascii_lowercase().starts_with(&normalized);
        right_starts
            .cmp(&left_starts)
            .then_with(|| left.relative_path.len().cmp(&right.relative_path.len()))
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    filtered.truncate(limit);
    filtered
}

pub fn apply_citation_candidate(
    input: &str,
    query: &ActiveCitationQuery,
    candidate: &CitationCandidate,
) -> CitationApplyResult {
    let mut lines = split_lines_preserving_empty(input);
    let line = lines.get(query.row).cloned().unwrap_or_default();

    let prefix = line.chars().take(query.start_col).collect::<String>();
    let suffix = line.chars().skip(query.end_col).collect::<String>();
    let replacement = format_citation_markdown(&PathBuf::from(&candidate.absolute_path))
        .unwrap_or_else(|| format!("[{}]({})", candidate.label, candidate.absolute_path));
    lines[query.row] = format!("{prefix}{replacement}{suffix}");

    CitationApplyResult {
        text: lines.join("\n"),
        cursor: (
            query.row,
            prefix.chars().count() + replacement.chars().count(),
        ),
    }
}

pub fn parse_render_spans(input: &str) -> Vec<CitationRenderSpan> {
    split_lines_preserving_empty(input)
        .into_iter()
        .enumerate()
        .flat_map(|(line_index, line)| parse_render_spans_in_line(&line, line_index))
        .collect()
}

pub fn normalize_pasted_path(pasted: &str) -> Option<PathBuf> {
    let pasted = pasted.trim();
    let unquoted = pasted
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| pasted.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(pasted);

    if let Ok(url) = Url::parse(unquoted) {
        if url.scheme() == "file" {
            return url.to_file_path().ok();
        }
    }

    if let Some(path) = normalize_windows_path(unquoted) {
        return Some(path);
    }

    let parts: Vec<String> = shlex::Shlex::new(pasted).collect();
    if parts.len() == 1 {
        let part = parts.into_iter().next()?;
        if let Some(path) = normalize_windows_path(&part) {
            return Some(path);
        }
        return Some(PathBuf::from(part));
    }

    None
}

pub fn format_citation_markdown(path: &Path) -> Option<String> {
    let absolute = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let label = absolute.file_name()?.to_str()?;
    Some(format!("[{}]({})", label, absolute.display()))
}

fn parse_render_spans_in_line(line: &str, line_index: usize) -> Vec<CitationRenderSpan> {
    let chars = line.chars().collect::<Vec<_>>();
    let mut spans = Vec::new();
    let mut i = 0usize;

    while i < chars.len() {
        if chars[i] != '[' {
            i += 1;
            continue;
        }

        let Some(label_end) = chars[i + 1..].iter().position(|ch| *ch == ']') else {
            break;
        };
        let label_end = i + 1 + label_end;
        if chars.get(label_end + 1) != Some(&'(') {
            i += 1;
            continue;
        }
        let Some(target_end) = chars[label_end + 2..].iter().position(|ch| *ch == ')') else {
            break;
        };
        let target_end = label_end + 2 + target_end;

        let label = chars[i + 1..label_end].iter().collect::<String>();
        let target = chars[label_end + 2..target_end].iter().collect::<String>();
        if label.is_empty() || target.is_empty() {
            i += 1;
            continue;
        }

        spans.push(CitationRenderSpan {
            line_index,
            start_col: i,
            end_col: target_end + 1,
            label,
            target,
        });
        i = target_end + 1;
    }

    spans
}

fn split_lines_preserving_empty(input: &str) -> Vec<String> {
    let lines = input.lines().map(str::to_string).collect::<Vec<_>>();
    if lines.is_empty() {
        vec![String::new()]
    } else {
        lines
    }
}

fn normalize_windows_path(input: &str) -> Option<PathBuf> {
    let drive = input
        .chars()
        .next()
        .map(|c| c.is_ascii_alphabetic())
        .unwrap_or(false)
        && input.get(1..2) == Some(":")
        && input
            .get(2..3)
            .map(|s| s == "\\" || s == "/")
            .unwrap_or(false);
    let unc = input.starts_with("\\\\");
    if !drive && !unc {
        return None;
    }
    Some(PathBuf::from(input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_active_citation_query_at_cursor() {
        let query = find_active_citation_query("look at @revi", (0, 13)).expect("query");
        assert_eq!(query.query, "revi");
        assert_eq!(query.start_col, 8);
        assert_eq!(query.end_col, 13);
    }

    #[test]
    fn ignores_embedded_at_without_token_boundary() {
        assert!(find_active_citation_query("email@test", (0, 10)).is_none());
    }

    #[test]
    fn filters_candidates_by_label_and_path() {
        let candidates = vec![
            CitationCandidate {
                label: "reviewer.md".to_string(),
                absolute_path: "/tmp/reviewer.md".to_string(),
                relative_path: "docs/reviewer.md".to_string(),
            },
            CitationCandidate {
                label: "plan.md".to_string(),
                absolute_path: "/tmp/plan.md".to_string(),
                relative_path: "docs/plans/plan.md".to_string(),
            },
        ];

        let filtered = filter_citation_candidates(&candidates, "revi", 10);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].label, "reviewer.md");
    }

    #[test]
    fn scan_workspace_citation_candidates_includes_directories() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("docs/guides")).expect("mkdirs");
        std::fs::write(temp.path().join("docs/guides/reviewer.md"), "review").expect("write file");

        let candidates =
            scan_workspace_citation_candidates(temp.path()).expect("scan workspace citations");

        assert!(candidates.iter().any(|candidate| candidate.label == "docs"));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.label == "guides"));
        assert!(candidates
            .iter()
            .any(|candidate| candidate.relative_path == "docs/guides/reviewer.md"));
    }

    #[test]
    fn apply_candidate_replaces_only_active_query() {
        let candidate = CitationCandidate {
            label: "reviewer.md".to_string(),
            absolute_path: "/tmp/reviewer.md".to_string(),
            relative_path: "reviewer.md".to_string(),
        };
        let query = find_active_citation_query("use @rev now", (0, 8)).expect("query");

        let applied = apply_citation_candidate("use @rev now", &query, &candidate);

        assert_eq!(applied.text, "use [reviewer.md](/tmp/reviewer.md) now");
    }

    #[test]
    fn parse_render_spans_extracts_markdown_links() {
        let spans = parse_render_spans("Use [reviewer.md](/tmp/reviewer.md) now");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].label, "reviewer.md");
        assert_eq!(spans[0].target, "/tmp/reviewer.md");
    }

    #[test]
    fn normalize_pasted_file_url() {
        let path = normalize_pasted_path("file:///tmp/reviewer.md").expect("path");
        assert_eq!(path, PathBuf::from("/tmp/reviewer.md"));
    }

    #[test]
    fn normalize_pasted_quoted_path() {
        let path = normalize_pasted_path("\"/tmp/reviewer.md\"").expect("path");
        assert_eq!(path, PathBuf::from("/tmp/reviewer.md"));
    }
}
