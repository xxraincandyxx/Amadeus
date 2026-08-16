// @amadeus-header
// summary: Scenario tags, matching rules, and similarity scoring for cross-scenario preference resolution.
// layer: core
// status: active
// feature_flags: none
// provides:
// - type: crate::preference::scenario::ScenarioTag
// - fn: crate::preference::scenario::scenario_similarity
// - fn: crate::preference::scenario::scenario_matches
// uses:
// - format: serde serialization
// invariants:
// - The wildcard name "" or "*" matches every scenario.
// - scenario_similarity is symmetric and bounded in [0.0, 1.0].
// side_effects: none
// tests:
// - cmd: cargo test -p context
// @end-amadeus-header

//! Scenario tagging and matching used to attach preferences to contexts
//! and to reuse them across similar scenarios.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A lightweight scenario context attached to a preference.
///
/// `name` is the coarse scenario (e.g. `code_review`, `file_ops`); `attrs`
/// carries finer key/value context (e.g. `{"fs": "local"}`). An empty name
/// or `"*"` acts as a wildcard that matches every scenario.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioTag {
    pub name: String,
    #[serde(default)]
    pub attrs: BTreeMap<String, String>,
}

impl ScenarioTag {
    /// Build a scenario with only a name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            attrs: BTreeMap::new(),
        }
    }

    /// Chainable attribute setter.
    pub fn with_attr(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.insert(key.into(), value.into());
        self
    }

    /// Whether this tag is a wildcard that applies to every scenario.
    pub fn is_any(&self) -> bool {
        self.name.is_empty() || self.name == "*"
    }
}

/// Whether `rule` (a preference's scenario) is satisfied by `runtime` (the
/// scenario currently in effect): either `rule` is a wildcard, or both names
/// agree and every attribute of `rule` is present with the same value in
/// `runtime`.
pub fn scenario_matches(rule: &ScenarioTag, runtime: &ScenarioTag) -> bool {
    if !rule.is_any() && rule.name != runtime.name {
        return false;
    }
    rule.attrs
        .iter()
        .all(|(k, v)| runtime.attrs.get(k) == Some(v))
}

/// Symmetric similarity in `[0.0, 1.0]` between two scenarios.
///
/// A matching name dominates (base 0.7, boosted by attribute overlap); with
/// different names only attribute overlap contributes, scaled down so exact
/// scenario hits always outrank cross-scenario ones.
pub fn scenario_similarity(a: &ScenarioTag, b: &ScenarioTag) -> f32 {
    if a == b {
        return 1.0;
    }
    if a.is_any() || b.is_any() {
        return 0.0;
    }
    let name_hit = a.name == b.name;
    let attr_sim = jaccard_attrs(&a.attrs, &b.attrs);
    if name_hit {
        (0.7 + 0.3 * attr_sim).min(1.0)
    } else {
        0.35 * attr_sim
    }
}

fn jaccard_attrs(a: &BTreeMap<String, String>, b: &BTreeMap<String, String>) -> f32 {
    let mut inter = 0usize;
    for (k, v) in a {
        if b.get(k) == Some(v) {
            inter += 1;
        }
    }
    let union = a.len() + b.len() - inter;
    if union == 0 {
        return 0.0;
    }
    inter as f32 / union as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_scenarios_match_everything() {
        let any = ScenarioTag::new("*");
        assert!(any.is_any());
        assert!(scenario_matches(&any, &ScenarioTag::new("code_review")));

        let empty = ScenarioTag::default();
        assert!(empty.is_any());
        assert!(scenario_matches(&empty, &ScenarioTag::new("anything")));
    }

    #[test]
    fn exact_name_matches() {
        let rule = ScenarioTag::new("code_review");
        assert!(scenario_matches(&rule, &ScenarioTag::new("code_review")));
        assert!(!scenario_matches(
            &rule,
            &ScenarioTag::new("report_writing")
        ));
    }

    #[test]
    fn rule_attrs_must_be_satisfied_by_runtime() {
        let rule = ScenarioTag::new("file_ops").with_attr("fs", "local");
        let runtime = ScenarioTag::new("file_ops")
            .with_attr("fs", "local")
            .with_attr("mode", "edit");
        assert!(scenario_matches(&rule, &runtime));

        let remote = ScenarioTag::new("file_ops").with_attr("fs", "remote");
        assert!(!scenario_matches(&rule, &remote));
    }

    #[test]
    fn identical_scenarios_score_one() {
        let a = ScenarioTag::new("x").with_attr("k", "v");
        assert!((scenario_similarity(&a, &a) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn same_name_outranks_cross_scenario() {
        let a = ScenarioTag::new("report").with_attr("lang", "cn");
        let same_name = ScenarioTag::new("report").with_attr("lang", "en");
        let other_name = ScenarioTag::new("code").with_attr("lang", "cn");

        let same = scenario_similarity(&a, &same_name);
        let cross = scenario_similarity(&a, &other_name);
        assert!(
            same > cross,
            "same-name {same} should beat cross-name {cross}"
        );
        assert!(same >= 0.7);
        assert!(cross <= 0.35 + 1e-6);
    }

    #[test]
    fn disjoint_attrs_score_zero() {
        let a = ScenarioTag::new("a").with_attr("k1", "v1");
        let b = ScenarioTag::new("b").with_attr("k2", "v2");
        assert!(scenario_similarity(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn wildcard_never_counts_as_similar() {
        let any = ScenarioTag::new("*");
        let s = ScenarioTag::new("dev");
        assert!(scenario_similarity(&any, &s).abs() < 1e-6);
    }
}
