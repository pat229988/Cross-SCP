// SPDX-License-Identifier: AGPL-3.0-or-later

//! Clean-room file mask matching.
//!
//! Reference policy: legacy SFTP client `source/core/FileMasks.*` is a behavior reference
//! only. This implementation is intentionally small and tested from documented
//! expected behavior, not copied source.

/// A single glob-like file mask.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileMask {
    pattern: String,
}

impl FileMask {
    /// Build a mask from a pattern supporting `*` and `?` wildcards.
    #[must_use]
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
        }
    }

    /// Returns true when `path` matches this mask.
    #[must_use]
    pub fn matches(&self, path: &str) -> bool {
        wildcard_match(&self.pattern, path)
    }
}

/// Include/exclude decision after evaluating a mask set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaskDecision {
    Included,
    Excluded,
}

/// Ordered include/exclude masks. Exclusions win over inclusions.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MaskSet {
    includes: Vec<FileMask>,
    excludes: Vec<FileMask>,
}

impl MaskSet {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn include(mut self, pattern: impl Into<String>) -> Self {
        self.includes.push(FileMask::new(pattern));
        self
    }

    #[must_use]
    pub fn exclude(mut self, pattern: impl Into<String>) -> Self {
        self.excludes.push(FileMask::new(pattern));
        self
    }

    #[must_use]
    pub fn decide(&self, path: &str) -> MaskDecision {
        if self.excludes.iter().any(|mask| mask.matches(path)) {
            return MaskDecision::Excluded;
        }

        if self.includes.is_empty() || self.includes.iter().any(|mask| mask.matches(path)) {
            MaskDecision::Included
        } else {
            MaskDecision::Excluded
        }
    }
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.as_bytes();
    let text = text.as_bytes();
    let (mut p, mut t) = (0_usize, 0_usize);
    let mut star: Option<usize> = None;
    let mut star_text = 0_usize;

    while t < text.len() {
        if p < pattern.len() && (pattern[p] == b'?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            p += 1;
            star_text = t;
        } else if let Some(star_index) = star {
            p = star_index + 1;
            star_text += 1;
            t = star_text;
        } else {
            return false;
        }
    }

    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }

    p == pattern.len()
}

#[cfg(test)]
mod tests {
    use super::{FileMask, MaskDecision, MaskSet};

    #[test]
    fn exact_masks_match_exact_paths_only() {
        let mask = FileMask::new("report.txt");

        assert!(mask.matches("report.txt"));
        assert!(!mask.matches("report.csv"));
        assert!(!mask.matches("old/report.txt"));
    }

    #[test]
    fn star_matches_zero_or_more_characters() {
        let mask = FileMask::new("*.txt");

        assert!(mask.matches("report.txt"));
        assert!(mask.matches(".txt"));
        assert!(!mask.matches("report.txt.bak"));
    }

    #[test]
    fn question_mark_matches_one_character() {
        let mask = FileMask::new("file-?.log");

        assert!(mask.matches("file-1.log"));
        assert!(!mask.matches("file-10.log"));
        assert!(!mask.matches("file-.log"));
    }

    #[test]
    fn exclude_masks_win_over_include_masks() {
        let masks = MaskSet::new().include("*.txt").exclude("secret-*.txt");

        assert_eq!(masks.decide("notes.txt"), MaskDecision::Included);
        assert_eq!(masks.decide("secret-plan.txt"), MaskDecision::Excluded);
        assert_eq!(masks.decide("image.png"), MaskDecision::Excluded);
    }

    #[test]
    fn empty_include_set_includes_by_default() {
        let masks = MaskSet::new().exclude("*.tmp");

        assert_eq!(masks.decide("keep.md"), MaskDecision::Included);
        assert_eq!(masks.decide("scratch.tmp"), MaskDecision::Excluded);
    }
}
