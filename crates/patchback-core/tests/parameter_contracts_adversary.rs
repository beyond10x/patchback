//! Adversarial cases for story:parameter-contracts (wave a2). Each case states the DESIGN.md or
//! domain clause it drives the implementation from.

use patchback_core::parameter::{
    ClampPolicy, ParameterContract, ParameterMapping, ParameterUnit, SUBTRACTIVE_MONO,
    SetParameterError, Topology, TopologyError,
};

fn contract(
    path: &str,
    index: usize,
    mapping: ParameterMapping,
    low: f64,
    high: f64,
) -> ParameterContract {
    ParameterContract {
        index,
        path: path.to_owned(),
        unit: ParameterUnit::Dimensionless,
        mapping,
        physical_at_zero: low,
        physical_at_one: high,
        default_normalized: 0.5,
        clamp: ClampPolicy::ClampToUnitInterval,
    }
}

/// § 2 invariant 4 (never non-finite output) and the module's promise that a validated contract is
/// usable: a contract `Topology::new` accepts must map every `p` in `[0, 1]`, its default included,
/// to a finite physical value. Otherwise validation must refuse it.
fn assert_accepted_contract_maps_finitely(candidate: ParameterContract) {
    let path = candidate.path.clone();
    let Ok(topology) = Topology::new(SUBTRACTIVE_MONO, vec![candidate]) else {
        return;
    };
    let accepted = topology.contract(&path).unwrap();
    for p in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let setting = topology.set(&path, p).unwrap();
        assert!(
            setting.physical.is_finite(),
            "accepted contract {accepted:?} maps p = {p} to {}",
            setting.physical
        );
    }
    assert!(accepted.default_physical().is_finite());
}

#[test]
fn adversary_a_finite_linear_range_whose_width_overflows_is_refused_or_maps_finitely() {
    assert_accepted_contract_maps_finitely(contract(
        "wide",
        0,
        ParameterMapping::Linear,
        -1e308,
        1e308,
    ));
}

#[test]
fn adversary_an_exponential_range_whose_ratio_overflows_is_refused_or_maps_finitely() {
    assert_accepted_contract_maps_finitely(contract(
        "subnormal_low",
        0,
        ParameterMapping::Exponential,
        f64::from_bits(1),
        1.0,
    ));
}

/// § 5.2: serialized values are physical, model targets normalized. A physical value the contract
/// itself produced, read back through the inverse and set again, lies in `[0, 1]` and so must not
/// raise a § 8.1 clamp warning; and the physical value lies inside the declared range.
#[test]
fn adversary_physical_values_the_contract_produced_stay_in_range_and_reset_without_a_clamp() {
    let topology = Topology::subtractive_mono();
    for contract in topology.contracts() {
        for i in 0..=1000 {
            let p = i as f64 / 1000.0;
            let physical = topology.set(&contract.path, p).unwrap().physical;
            assert!(
                (contract.physical_at_zero..=contract.physical_at_one).contains(&physical),
                "{} at p = {p}: {physical} outside [{}, {}]",
                contract.path,
                contract.physical_at_zero,
                contract.physical_at_one
            );
            let back = contract.to_normalized(physical).unwrap();
            let again = topology.set(&contract.path, back).unwrap();
            assert_eq!(
                again.clamped, None,
                "{} at p = {p}: physical {physical} reads back as {back}",
                contract.path
            );
        }
    }
}

/// `patchback.patch.DeclareTopology` orders duplicate-path before duplicate-index before
/// invalid-contract. The unit's own tests never present two conflicts at once, so swapping the
/// checks in `Topology::new` stays green; these cases hold the order.
#[test]
fn adversary_refusal_order_follows_the_domain_when_conflicts_coincide() {
    let decay = |index| contract("decay", index, ParameterMapping::Exponential, 0.005, 2.0);
    assert_eq!(
        Topology::new(SUBTRACTIVE_MONO, vec![decay(1), decay(1)]),
        Err(TopologyError::DuplicateParameterPath {
            name: SUBTRACTIVE_MONO.to_owned(),
            path: "decay".to_owned()
        })
    );
    let mut invalid = decay(1);
    invalid.default_normalized = 1.5;
    assert_eq!(
        Topology::new(SUBTRACTIVE_MONO, vec![invalid.clone(), decay(1)]),
        Err(TopologyError::DuplicateParameterPath {
            name: SUBTRACTIVE_MONO.to_owned(),
            path: "decay".to_owned()
        })
    );
    let mut release = contract("release", 1, ParameterMapping::Exponential, 0.005, 2.0);
    release.default_normalized = 1.5;
    assert_eq!(
        Topology::new(SUBTRACTIVE_MONO, vec![decay(1), release]),
        Err(TopologyError::DuplicateParameterIndex {
            name: SUBTRACTIVE_MONO.to_owned(),
            index: 1
        })
    );
}

/// § 8.1 "clamp with an explicit warning": the smallest values outside `[0, 1]` are clamped and
/// recorded; -0.0 is inside the interval and maps to the lower end exactly.
#[test]
fn adversary_tiny_excursions_are_recorded_and_negative_zero_is_not_a_clamp() {
    let topology = Topology::subtractive_mono();
    for submitted in [-f64::from_bits(1), 1.0 + f64::EPSILON, -f64::MIN_POSITIVE] {
        for contract in topology.contracts() {
            let setting = topology.set(&contract.path, submitted).unwrap();
            let record = setting
                .clamped
                .unwrap_or_else(|| panic!("{} {submitted:e}: silent", contract.path));
            assert_eq!(record.submitted.to_bits(), submitted.to_bits());
        }
    }
    for contract in topology.contracts() {
        let setting = topology.set(&contract.path, -0.0).unwrap();
        assert_eq!(setting.clamped, None, "{}", contract.path);
        assert_eq!(
            setting.physical, contract.physical_at_zero,
            "{}",
            contract.path
        );
    }
}

/// The unknown path is refused before the value is looked at, whatever the value.
#[test]
fn adversary_an_unknown_path_is_refused_whatever_the_value() {
    let topology = Topology::subtractive_mono();
    for value in [1.4, -0.2, f64::NAN, f64::INFINITY] {
        assert_eq!(
            topology.set("lfo_rate", value),
            Err(SetParameterError::UnknownParameterPath {
                path: "lfo_rate".to_owned()
            }),
            "{value}"
        );
    }
}
