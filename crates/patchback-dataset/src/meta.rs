//! The text files of a generation (D-a6-13): `dataset.meta`, what was generated and how, and one
//! `samples.meta` a split, its samples in split order. Both are line-oriented and versioned like
//! the `.patch` and `notes.events` files: a header line `<format> <version>`, then one record a
//! line, fields separated by one space, each line ending in `\n`.
//!
//! `dataset.meta` (`patchback-dataset-meta 1`) holds these records, once each, in this order:
//!
//! | Record | Value |
//! |---|---|
//! | `seed` | the dataset seed |
//! | `prng` | the PRNG algorithm, `xoshiro256**` |
//! | `batch_size` | `16` |
//! | `feature_contract` | `features/1` |
//! | `topology` | `subtractive-mono/1` |
//! | `train`, `validation`, `test` | the split's sample count |
//! | `diagnostic_silence`, `diagnostic_noise`, `diagnostic_clipping` | the diagnostic examples of the kind |
//!
//! `samples.meta` (`patchback-dataset-samples 1`) holds one line a sample, in the order the
//! samples were drawn, `<patch_seed> <midi_note> <gate_samples>`, unsigned decimal integers. Row
//! `i` is the `i`th row of the split's dense files (story:dense-data-files).

use patchback_analysis::FEATURES_1_VERSION;

use crate::generate::Generated;
use crate::model::{DiagnosticKind, SplitRole};

/// The header of `dataset.meta`.
pub const DATASET_META_HEADER: &str = "patchback-dataset-meta";

/// The header of `samples.meta`.
pub const SAMPLES_META_HEADER: &str = "patchback-dataset-samples";

/// The version of both formats.
pub const META_FORMAT_VERSION: u32 = 1;

/// The file name of a dataset's metadata, in its output directory.
pub const DATASET_META_FILE: &str = "dataset.meta";

/// The file name of a split's sample list, in the split's directory ([`SplitRole::dir`]).
pub const SAMPLES_META_FILE: &str = "samples.meta";

/// The split and diagnostic counts as `key value` lines: the last six records of `dataset.meta`,
/// and what `patchback generate-dataset` prints after the output directory.
pub fn count_records(generated: &Generated) -> String {
    let mut text = String::new();
    for role in SplitRole::ALL {
        text.push_str(&format!(
            "{} {}\n",
            role.key(),
            generated.samples(role).len()
        ));
    }
    for kind in DiagnosticKind::ALL {
        text.push_str(&format!(
            "diagnostic_{} {}\n",
            kind.key(),
            generated.diagnostic_count(kind)
        ));
    }
    text
}

/// The text of `dataset.meta`.
pub fn dataset_meta(generated: &Generated) -> String {
    let dataset = generated.dataset();
    format!(
        "{DATASET_META_HEADER} {META_FORMAT_VERSION}\nseed {}\nprng {}\nbatch_size {}\n\
         feature_contract {FEATURES_1_VERSION}\ntopology {}\n{}",
        dataset.seed,
        dataset.prng_algorithm.0,
        dataset.batch_size,
        dataset.topology_id,
        count_records(generated)
    )
}

/// The text of the `samples.meta` of the split of `role`.
pub fn samples_meta(generated: &Generated, role: SplitRole) -> String {
    let mut text = format!("{SAMPLES_META_HEADER} {META_FORMAT_VERSION}\n");
    for sample in generated.samples(role) {
        text.push_str(&format!(
            "{} {} {}\n",
            generated.patch_seed_of(sample),
            sample.midi_note,
            sample.gate_samples
        ));
    }
    text
}
