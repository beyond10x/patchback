//! The role of a dataset split (`patchback.dataset.SplitRole`, DESIGN.md § 7.1), shared by the
//! dataset crate that draws the splits and the analysis crate that computes normalization
//! statistics from the training split only (§ 6.3). It lives here because `patchback-dataset`
//! depends on `patchback-analysis`, so the analysis crate cannot read the type from the dataset
//! crate; `patchback_dataset::SplitRole` re-exports it.

use std::fmt;

/// `patchback.dataset.SplitRole`: the three splits of § 7.1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SplitRole {
    Train,
    Validation,
    Test,
}

impl SplitRole {
    /// The roles in the order the generator fills them.
    pub const ALL: [Self; 3] = [Self::Train, Self::Validation, Self::Test];

    /// The variant name of the model.
    pub fn name(self) -> &'static str {
        match self {
            Self::Train => "Train",
            Self::Validation => "Validation",
            Self::Test => "Test",
        }
    }

    /// The split's record name: its key in `dataset.meta` and in what `patchback generate-dataset`
    /// prints (`train`, `validation`, `test`). Not its directory: see [`dir`](Self::dir).
    pub fn key(self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Validation => "validation",
            Self::Test => "test",
        }
    }

    /// The split's directory under a dataset's output directory (`train`, `val`, `test`; DESIGN.md
    /// § 7.2, `data/val/x.f32`), where its `samples.meta` and dense files go.
    pub fn dir(self) -> &'static str {
        match self {
            Self::Train => "train",
            Self::Validation => "val",
            Self::Test => "test",
        }
    }
}

impl fmt::Display for SplitRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::SplitRole;

    #[test]
    fn the_three_roles_keep_their_names_keys_and_directories() {
        let rows: [(SplitRole, &str, &str, &str); 3] = [
            (SplitRole::Train, "Train", "train", "train"),
            (SplitRole::Validation, "Validation", "validation", "val"),
            (SplitRole::Test, "Test", "test", "test"),
        ];
        for (i, (role, name, key, dir)) in rows.iter().enumerate() {
            assert_eq!(SplitRole::ALL[i], *role);
            assert_eq!(role.name(), *name);
            assert_eq!(role.key(), *key);
            assert_eq!(role.dir(), *dir);
            assert_eq!(role.to_string(), *name);
        }
    }
}
