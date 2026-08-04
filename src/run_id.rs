use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt;
use ulid::Ulid;

/// A durable run's unique identity, backed by a canonical ULID.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunId(Ulid);

/// Shortest prefix length a display ever collapses a run to before lengthening.
const MIN_DISPLAY_LEN: usize = 4;

impl RunId {
    pub fn generate() -> Self {
        Self(Ulid::generate())
    }

    pub fn parse(value: &str) -> Result<Self> {
        let ulid = Ulid::from_string(value)
            .map_err(|error| anyhow::anyhow!("run.id is not a canonical ULID: {error}"))?;
        Ok(Self(ulid))
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// Failure to resolve a query prefix to exactly one candidate run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrefixError {
    NotFound { prefix: String },
    Ambiguous { prefix: String },
}

impl fmt::Display for PrefixError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { prefix } => write!(formatter, "no run matches prefix '{prefix}'"),
            Self::Ambiguous { prefix } => {
                write!(formatter, "prefix '{prefix}' matches more than one run")
            }
        }
    }
}

impl std::error::Error for PrefixError {}

/// Resolve a query prefix to exactly one candidate, rejecting ambiguity and misses.
pub fn resolve_prefix(prefix: &str, candidates: &[RunId]) -> Result<RunId, PrefixError> {
    let needle = prefix.to_uppercase();
    let mut matches = candidates
        .iter()
        .filter(|candidate| candidate.to_string().starts_with(&needle));

    let Some(&first) = matches.next() else {
        return Err(PrefixError::NotFound {
            prefix: prefix.to_string(),
        });
    };
    match matches.next() {
        Some(_) => Err(PrefixError::Ambiguous {
            prefix: prefix.to_string(),
        }),
        None => Ok(first),
    }
}

/// Shortest prefix that uniquely identifies `target` among `all`.
pub fn shortest_unique_prefix(target: &RunId, all: &[RunId]) -> String {
    let canonical = target.to_string();
    let unique = |length: usize| {
        let candidate = &canonical[..length];
        all.iter()
            .filter(|other| other.to_string().starts_with(candidate))
            .count()
            == 1
    };

    let length = (MIN_DISPLAY_LEN..canonical.len())
        .find(|&length| unique(length))
        .unwrap_or(canonical.len());
    canonical[..length].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_id(value: u128) -> RunId {
        RunId(Ulid::from(value))
    }

    #[test]
    fn generate_produces_distinct_parseable_ids() {
        let first = RunId::generate();
        let second = RunId::generate();
        assert_ne!(first, second);

        let reparsed = RunId::parse(&first.to_string()).expect("generated id parses");
        assert_eq!(first, reparsed);
    }

    #[test]
    fn parse_round_trips_display() {
        let id = run_id(1);
        let text = id.to_string();
        assert_eq!(RunId::parse(&text).expect("canonical parses"), id);
    }

    #[test]
    fn parse_rejects_malformed() {
        assert!(RunId::parse("not-a-ulid").is_err());
    }

    #[test]
    fn resolve_prefix_returns_unique_match() {
        let target = run_id(1);
        let candidates = [target, run_id(1 << 80)];
        let prefix = &target.to_string()[..MIN_DISPLAY_LEN + 20];

        assert_eq!(resolve_prefix(prefix, &candidates), Ok(target));
    }

    #[test]
    fn resolve_prefix_rejects_ambiguous() {
        let candidates = [run_id(1), run_id(2)];
        let shared = &candidates[0].to_string()[..MIN_DISPLAY_LEN];

        assert_eq!(
            resolve_prefix(shared, &candidates),
            Err(PrefixError::Ambiguous {
                prefix: shared.to_string(),
            })
        );
    }

    #[test]
    fn resolve_prefix_rejects_unknown() {
        let candidates = [run_id(1)];
        let miss = "ZZZZZZ";

        assert_eq!(
            resolve_prefix(miss, &candidates),
            Err(PrefixError::NotFound {
                prefix: miss.to_string(),
            })
        );
    }

    #[test]
    fn shortest_unique_prefix_uses_minimum_when_alone() {
        let target = run_id(1);
        assert_eq!(
            shortest_unique_prefix(&target, &[target]).len(),
            MIN_DISPLAY_LEN
        );
    }

    #[test]
    fn shortest_unique_prefix_lengthens_on_collision() {
        let target = run_id(1);
        let sibling = run_id(2);
        let all = [target, sibling];

        let prefix = shortest_unique_prefix(&target, &all);
        assert!(
            prefix.len() > MIN_DISPLAY_LEN,
            "collision must lengthen prefix"
        );
        assert!(target.to_string().starts_with(&prefix));
        assert!(!sibling.to_string().starts_with(&prefix));
    }
}
