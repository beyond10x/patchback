//! `ComputeNormalizationStats` over the splits of a [`DatasetStore`] (Da8-12, Da8-16;
//! story:normalization-stats). The analysis crate computes the statistics from a split's role and
//! rows ([`patchback_analysis::compute_normalization_stats`]); this module reads them from the
//! store: the split by its identity ([`DatasetStore::split`], `None` when the store holds none, so
//! the model's first outcome, no such split, is answered first) and its samples' `x` values in
//! the order they were added ([`DatasetStore::samples_in`]), the split in any state (Da8-13).

use patchback_analysis::{
    FeatureContractDefinition, FeatureContractId, FeatureRegistry, NormalizationError,
    NormalizationStats, SplitInput, compute_normalization_stats,
};

use crate::{DatasetSplitId, DatasetStore, Generated, SplitRole};

/// `ComputeNormalizationStats` of the split `dataset_split_id` of `store`, under the contract
/// `feature_contract_id` of `registry` and the version `feature_contract_version`, refused in the
/// model's order ([`patchback_analysis::compute_normalization_stats`]). Nothing compares the
/// contract with the one the split's dataset was generated under (Da8-17).
pub fn compute_split_stats(
    store: &DatasetStore,
    registry: &FeatureRegistry,
    feature_contract_id: FeatureContractId,
    dataset_split_id: DatasetSplitId,
    feature_contract_version: &str,
) -> Result<NormalizationStats, NormalizationError> {
    let split = store.split(dataset_split_id);
    let rows: Vec<[f64; 32]> = match split {
        Some(_) => store
            .samples_in(dataset_split_id)
            .iter()
            .map(|sample| sample.x)
            .collect(),
        None => Vec::new(),
    };
    compute_normalization_stats(
        registry,
        feature_contract_id,
        feature_contract_version,
        dataset_split_id.0,
        split.map(|split| SplitInput {
            role: split.role,
            rows: &rows,
        }),
    )
}

/// The statistics of a generation's training split under its dataset's contract, `features/1`,
/// as `patchback generate-dataset` writes them: the contract is defined and published in a
/// registry of its own, as the generator's `Pipeline` holds it, so it has the identity the
/// dataset records (a registry that gave it another would answer `FeatureContractNotFound`, and
/// an unpublished one `FeatureContractNotPublished`).
pub fn training_stats(generated: &Generated) -> Result<NormalizationStats, NormalizationError> {
    let definition = FeatureContractDefinition::features_1();
    let version = definition.version.clone();
    let mut registry = FeatureRegistry::new();
    if let Ok(id) = registry.define(definition) {
        let _ = registry.publish(id);
    }
    compute_split_stats(
        &generated.store,
        &registry,
        generated.dataset().feature_contract_id,
        generated.split_id(SplitRole::Train),
        &version,
    )
}
