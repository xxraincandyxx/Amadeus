// @amadeus-header
// summary: Shared identifier primitives reused across runtime surfaces.
// layer: core
// status: active
// feature_flags: none
// provides:
// - module: crate
// - type: crate::AgentId
// - type: crate::TeamId
// - type: crate::CommitId
// - type: crate::TxId
// - type: crate::SnapshotId
// uses:
// - protocol: serde serialization
// - runtime: UUID generation and parsing
// invariants:
// - Identifier prefixes remain stable across transports and persisted state.
// side_effects: none
// tests:
// - cmd: cargo test -p ids
// @end-amadeus-header

//! Shared identifier primitives.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn nil() -> Self {
                Self(Uuid::nil())
            }

            pub fn is_nil(&self) -> bool {
                self.0.is_nil()
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", $prefix, self.0)
            }
        }

        impl FromStr for $name {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let parts: Vec<&str> = s.split(':').collect();
                let uuid_str = if parts.len() == 2 {
                    if parts[0] != $prefix {
                        return Err(format!(
                            "Invalid {} prefix: expected '{}', got '{}'",
                            stringify!($name),
                            $prefix,
                            parts[0]
                        ));
                    }
                    parts[1]
                } else {
                    s
                };
                let uuid = Uuid::parse_str(uuid_str).map_err(|e| format!("Invalid UUID: {}", e))?;
                Ok(Self(uuid))
            }
        }

        impl From<Uuid> for $name {
            fn from(uuid: Uuid) -> Self {
                Self(uuid)
            }
        }

        impl From<$name> for Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }
    };
}

define_id!(AgentId, "agent");
define_id!(TeamId, "team");
define_id!(CommitId, "commit");
define_id!(TxId, "tx");
define_id!(SnapshotId, "snapshot");

impl AgentId {
    pub fn system() -> Self {
        Self(Uuid::nil())
    }

    pub fn is_system(&self) -> bool {
        self.0.is_nil()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_id_display_uses_prefix() {
        let id = AgentId::new();
        let display = format!("{}", id);
        assert!(display.starts_with("agent:"));
    }

    #[test]
    fn agent_id_round_trips_from_string() {
        let id = AgentId::new();
        let display = format!("{}", id);
        let parsed: AgentId = display.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn system_agent_is_nil() {
        let sys = AgentId::system();
        assert!(sys.is_system());
        assert!(sys.is_nil());
    }
}
