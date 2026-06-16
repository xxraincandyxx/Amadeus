// @amadeus-header
// summary: Canonical path resolution policy shared by permissions and filesystem tools.
// layer: policy
// status: active
// feature_flags: none
// provides:
// - module: crate::security::path_policy
// - type: crate::security::path_policy::PathPolicy
// uses:
// - module: crate::error
// - artifact: filesystem paths and files
// invariants:
// - Path authorization and path execution use identical workspace-root semantics.
// side_effects:
// - Reads filesystem metadata for canonicalization.
// tests:
// - cmd: cargo test -p core security --features full
// @end-amadeus-header

use std::path::{Component, Path, PathBuf};

use crate::error::{AgentError, Result};

#[derive(Debug, Clone)]
pub struct PathPolicy {
    primary_root: PathBuf,
    roots: Vec<PathBuf>,
    protected_suffixes: Vec<&'static str>,
}

impl PathPolicy {
    pub fn new(primary_root: PathBuf, additional_roots: Vec<PathBuf>) -> Self {
        let mut roots = vec![primary_root.clone()];
        roots.extend(additional_roots);
        let roots = roots
            .into_iter()
            .map(|root| canonicalize_existing_or_self(&root))
            .collect();

        Self {
            primary_root: canonicalize_existing_or_self(&primary_root),
            roots,
            protected_suffixes: vec![".env", ".pem", ".key"],
        }
    }

    pub fn resolve_read(&self, input: &str) -> Result<PathBuf> {
        self.resolve(input, false)
    }

    pub fn resolve_write(&self, input: &str) -> Result<PathBuf> {
        let resolved = self.resolve(input, true)?;
        if self.is_protected(&resolved) {
            return Err(AgentError::PathEscape(PathBuf::from(input)));
        }
        Ok(resolved)
    }

    pub fn contains_path(&self, input: &str) -> bool {
        self.resolve(input, true).is_ok() || self.resolve(input, false).is_ok()
    }

    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    fn resolve(&self, input: &str, allow_missing_leaf: bool) -> Result<PathBuf> {
        let candidate = self.absolute_candidate(input)?;
        let resolved = if candidate.exists() {
            candidate
                .canonicalize()
                .map_err(|_| AgentError::PathEscape(PathBuf::from(input)))?
        } else if allow_missing_leaf {
            canonicalize_missing_target(&candidate)
                .ok_or_else(|| AgentError::PathEscape(PathBuf::from(input)))?
        } else {
            return Err(AgentError::PathEscape(PathBuf::from(input)));
        };

        if self.roots.iter().any(|root| resolved.starts_with(root)) {
            Ok(resolved)
        } else {
            Err(AgentError::PathEscape(PathBuf::from(input)))
        }
    }

    fn absolute_candidate(&self, input: &str) -> Result<PathBuf> {
        let path = Path::new(input);
        reject_parent_components(path)?;
        Ok(if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.primary_root.join(path)
        })
    }

    fn is_protected(&self, path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                self.protected_suffixes
                    .iter()
                    .any(|suffix| name.ends_with(suffix))
            })
    }
}

fn reject_parent_components(path: &Path) -> Result<()> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(AgentError::PathEscape(path.to_path_buf()));
    }
    Ok(())
}

fn canonicalize_existing_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn canonicalize_missing_target(path: &Path) -> Option<PathBuf> {
    let mut missing = Vec::new();
    let mut cursor = path;
    while !cursor.exists() {
        missing.push(cursor.file_name()?.to_os_string());
        cursor = cursor.parent()?;
    }

    let mut resolved = cursor.canonicalize().ok()?;
    for part in missing.iter().rev() {
        resolved.push(part);
    }
    Some(resolved)
}

#[cfg(test)]
mod tests {
    use super::PathPolicy;

    #[test]
    fn rejects_parent_escapes() {
        let dir = tempfile::tempdir().unwrap();
        let policy = PathPolicy::new(dir.path().to_path_buf(), Vec::new());
        assert!(policy.resolve_write("../escape.txt").is_err());
    }

    #[test]
    fn resolves_missing_write_target_inside_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let policy = PathPolicy::new(dir.path().to_path_buf(), Vec::new());
        let resolved = policy.resolve_write("nested/file.txt").unwrap();
        assert!(resolved.starts_with(dir.path().canonicalize().unwrap()));
    }

    #[test]
    fn denies_protected_write_target() {
        let dir = tempfile::tempdir().unwrap();
        let policy = PathPolicy::new(dir.path().to_path_buf(), Vec::new());
        assert!(policy.resolve_write("secret.env").is_err());
    }
}
