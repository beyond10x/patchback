//! The numeric obligations of story:parameter-contracts (DESIGN.md § 2 invariants 2 and 4,
//! § 5.2, § 8.1). The ESS scenarios `spec/scenarios/parameter-contracts-*.yaml` name the command
//! outcomes; these tests hold the numbers they cannot express.

use patchback_core::parameter::{
    ClampPolicy, InvalidContractReason, PLACEHOLDER_DEFAULT_NORMALIZED, ParameterClamped,
    ParameterContract, ParameterMapping, ParameterUnit, PhysicalValueError, SUBTRACTIVE_MONO,
    SetParameterError, Topology, TopologyError,
};

/// The f32 tolerance the story states: four units in the last place of an f32 at 1.0.
const TOLERANCE: f64 = 4.0 * f32::EPSILON as f64;

/// Grid points on [0, 1], both ends included.
const STEPS: usize = 1000;

fn grid() -> impl Iterator<Item = f64> {
    (0..=STEPS).map(|i| i as f64 / STEPS as f64)
}

/// One row of § 5.2: path, unit, mapping, physical value at p = 0 and at p = 1, and the
/// design's formula.
type DesignRow = (
    &'static str,
    ParameterUnit,
    ParameterMapping,
    f64,
    f64,
    fn(f64) -> f64,
);

/// A change that makes a valid contract invalid, and the reason the refusal must carry.
type Breakage = (fn(&mut ParameterContract), InvalidContractReason);

/// § 5.2, row by row.
const DESIGN: [DesignRow; 8] = [
    (
        "attack",
        ParameterUnit::Seconds,
        ParameterMapping::Exponential,
        0.001,
        2.0,
        |p| 0.001 * 2000f64.powf(p),
    ),
    (
        "decay",
        ParameterUnit::Seconds,
        ParameterMapping::Exponential,
        0.005,
        2.0,
        |p| 0.005 * 400f64.powf(p),
    ),
    (
        "sustain",
        ParameterUnit::LinearAmplitude,
        ParameterMapping::Linear,
        0.0,
        1.0,
        |p| p,
    ),
    (
        "release",
        ParameterUnit::Seconds,
        ParameterMapping::Exponential,
        0.005,
        2.0,
        |p| 0.005 * 400f64.powf(p),
    ),
    (
        "filter_cutoff",
        ParameterUnit::Hertz,
        ParameterMapping::Exponential,
        40.0,
        16000.0,
        |p| 40.0 * 400f64.powf(p),
    ),
    (
        "resonance_q",
        ParameterUnit::Dimensionless,
        ParameterMapping::Linear,
        0.5,
        12.0,
        |p| 0.5 + 11.5 * p,
    ),
    (
        "drive",
        ParameterUnit::Decibels,
        ParameterMapping::Linear,
        0.0,
        24.0,
        |p| 24.0 * p,
    ),
    (
        "output_gain",
        ParameterUnit::Decibels,
        ParameterMapping::Linear,
        -24.0,
        0.0,
        |p| -24.0 + 24.0 * p,
    ),
];

/// Whether `actual` is within the f32 tolerance of `expected`, relative to `scale`.
fn close(actual: f64, expected: f64, scale: f64) -> bool {
    (actual - expected).abs() <= TOLERANCE * scale.abs().max(1.0)
}

fn contract(path: &str, index: usize) -> ParameterContract {
    ParameterContract {
        index,
        path: path.to_owned(),
        unit: ParameterUnit::Seconds,
        mapping: ParameterMapping::Exponential,
        physical_at_zero: 0.005,
        physical_at_one: 2.0,
        default_normalized: 0.5,
        clamp: ClampPolicy::ClampToUnitInterval,
    }
}

#[test]
fn subtractive_mono_declares_the_eight_contracts_in_design_order() {
    let topology = Topology::subtractive_mono();
    assert_eq!(topology.name(), SUBTRACTIVE_MONO);
    assert_eq!(topology.name(), "subtractive-mono/1");
    let contracts = topology.contracts();
    assert_eq!(contracts.len(), 8);
    for (index, (contract, (path, unit, mapping, at_zero, at_one, _))) in
        contracts.iter().zip(DESIGN).enumerate()
    {
        assert_eq!(contract.index, index, "{path}");
        assert_eq!(contract.path, path);
        assert_eq!(contract.unit, unit, "{path}");
        assert_eq!(contract.mapping, mapping, "{path}");
        assert_eq!(contract.physical_at_zero, at_zero, "{path}");
        assert_eq!(contract.physical_at_one, at_one, "{path}");
        assert_eq!(contract.clamp, ClampPolicy::ClampToUnitInterval, "{path}");
        assert_eq!(contract.validate(), Ok(()), "{path}");
        assert_eq!(topology.contract(path), Some(contract));
    }
    assert_eq!(
        Topology::new(SUBTRACTIVE_MONO, contracts.to_vec()),
        Ok(topology.clone())
    );
}

#[test]
fn defaults_are_the_documented_placeholders_pending_design() {
    for contract in Topology::subtractive_mono().contracts() {
        assert_eq!(
            contract.default_normalized, PLACEHOLDER_DEFAULT_NORMALIZED,
            "{}",
            contract.path
        );
        assert_eq!(
            contract.default_physical(),
            contract.to_physical(PLACEHOLDER_DEFAULT_NORMALIZED),
            "{}",
            contract.path
        );
    }
}

#[test]
fn forward_mapping_follows_the_design_formulas() {
    let topology = Topology::subtractive_mono();
    for (path, _, _, at_zero, at_one, formula) in DESIGN {
        let contract = topology.contract(path).unwrap();
        for p in grid() {
            let physical = contract.to_physical(p);
            assert!(
                close(physical, formula(p), at_zero.abs().max(at_one.abs())),
                "{path} at p = {p}: {physical} against {}",
                formula(p)
            );
        }
    }
}

#[test]
fn endpoints_hit_the_design_ranges() {
    let topology = Topology::subtractive_mono();
    for (path, _, _, at_zero, at_one, _) in DESIGN {
        let contract = topology.contract(path).unwrap();
        let scale = at_zero.abs().max(at_one.abs());
        assert!(
            close(contract.to_physical(0.0), at_zero, scale),
            "{path} at 0"
        );
        assert!(
            close(contract.to_physical(1.0), at_one, scale),
            "{path} at 1"
        );
        assert!(
            close(contract.to_normalized(at_zero).unwrap(), 0.0, 1.0),
            "{path} inverse at 0"
        );
        assert!(
            close(contract.to_normalized(at_one).unwrap(), 1.0, 1.0),
            "{path} inverse at 1"
        );
    }
}

#[test]
fn inverse_of_forward_roundtrips_across_the_unit_interval() {
    for contract in Topology::subtractive_mono().contracts() {
        for p in grid() {
            let back = contract.to_normalized(contract.to_physical(p)).unwrap();
            assert!(close(back, p, 1.0), "{} at p = {p}: {back}", contract.path);
        }
    }
}

#[test]
fn forward_of_inverse_roundtrips_across_the_physical_range() {
    for contract in Topology::subtractive_mono().contracts() {
        let (low, high) = (contract.physical_at_zero, contract.physical_at_one);
        let scale = low.abs().max(high.abs());
        for t in grid() {
            let physical = low + (high - low) * t;
            let p = contract.to_normalized(physical).unwrap();
            assert!(
                (-TOLERANCE..=1.0 + TOLERANCE).contains(&p),
                "{} {physical}",
                contract.path
            );
            let back = contract.to_physical(p);
            assert!(
                close(back, physical, scale),
                "{} at {physical}: {back}",
                contract.path
            );
        }
    }
}

#[test]
fn mappings_are_strictly_increasing() {
    for contract in Topology::subtractive_mono().contracts() {
        let mut previous = f64::NEG_INFINITY;
        for p in grid() {
            let physical = contract.to_physical(p);
            assert!(physical > previous, "{} at p = {p}", contract.path);
            previous = physical;
        }
    }
}

#[test]
fn a_value_inside_the_unit_interval_is_set_as_given() {
    let topology = Topology::subtractive_mono();
    let setting = topology.set("filter_cutoff", 0.25).unwrap();
    assert_eq!(setting.path, "filter_cutoff");
    assert_eq!(setting.normalized, 0.25);
    assert!(close(setting.physical, 40.0 * 400f64.powf(0.25), 16000.0));
    assert_eq!(setting.clamped, None);
    for edge in [0.0, 1.0] {
        assert_eq!(topology.set("drive", edge).unwrap().clamped, None);
    }
}

#[test]
fn a_value_outside_the_unit_interval_is_clamped_and_the_clamp_recorded() {
    let topology = Topology::subtractive_mono();
    for (submitted, applied, physical) in [
        (1.4, 1.0, 16000.0),
        (-0.2, 0.0, 40.0),
        (f64::MAX, 1.0, 16000.0),
        (-f64::MAX, 0.0, 40.0),
        (1.0 + f64::EPSILON, 1.0, 16000.0),
    ] {
        let setting = topology.set("filter_cutoff", submitted).unwrap();
        assert_eq!(setting.normalized, applied, "{submitted}");
        assert!(close(setting.physical, physical, 16000.0), "{submitted}");
        assert_eq!(
            setting.clamped,
            Some(ParameterClamped {
                path: "filter_cutoff".to_owned(),
                submitted,
                applied,
            }),
            "{submitted}"
        );
    }
}

#[test]
fn a_non_finite_value_is_refused_naming_the_parameter() {
    let topology = Topology::subtractive_mono();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        match topology.set("attack", value) {
            Err(SetParameterError::NonFiniteParameterValue { path, value: given }) => {
                assert_eq!(path, "attack");
                assert_eq!(given.to_bits(), value.to_bits());
            }
            other => panic!("{value}: {other:?}"),
        }
        let contract = topology.contract("attack").unwrap();
        assert!(matches!(
            contract.apply(value),
            Err(SetParameterError::NonFiniteParameterValue { .. })
        ));
    }
}

#[test]
fn a_path_outside_the_topology_is_refused() {
    assert_eq!(
        Topology::subtractive_mono().set("lfo_rate", 0.5),
        Err(SetParameterError::UnknownParameterPath {
            path: "lfo_rate".to_owned()
        })
    );
    assert_eq!(Topology::subtractive_mono().contract("adsr.attack"), None);
}

#[test]
fn the_inverse_refuses_values_no_normalized_value_maps_to() {
    let topology = Topology::subtractive_mono();
    let cutoff = topology.contract("filter_cutoff").unwrap();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            cutoff.to_normalized(value),
            Err(PhysicalValueError::NonFinite { .. })
        ));
    }
    for value in [0.0, -40.0] {
        assert_eq!(
            cutoff.to_normalized(value),
            Err(PhysicalValueError::OutsideExponentialDomain {
                path: "filter_cutoff".to_owned(),
                value
            })
        );
    }
    let gain = topology.contract("output_gain").unwrap();
    assert!(close(gain.to_normalized(-48.0).unwrap(), -1.0, 1.0));
}

/// A topology name other than `subtractive-mono/1`, so a refusal is seen to carry the name it was
/// given rather than a constant.
const OTHER: &str = "test-voice/1";

#[test]
fn an_invalid_contract_is_refused_with_a_typed_reason() {
    use InvalidContractReason::*;
    let cases: [Breakage; 11] = [
        (|c| c.default_normalized = 1.5, DefaultOutsideUnitInterval),
        (|c| c.default_normalized = -0.1, DefaultOutsideUnitInterval),
        (
            |c| c.default_normalized = f64::NAN,
            DefaultOutsideUnitInterval,
        ),
        (|c| c.physical_at_zero = 0.0, ExponentialDomainNotPositive),
        (|c| c.physical_at_zero = -1.0, ExponentialDomainNotPositive),
        (|c| c.physical_at_one = c.physical_at_zero, EmptyDomain),
        (|c| c.physical_at_one = 0.001, EmptyDomain),
        (|c| c.physical_at_zero = f64::NAN, NonFiniteBound),
        (|c| c.physical_at_one = f64::INFINITY, NonFiniteBound),
        (
            |c| {
                c.mapping = ParameterMapping::Linear;
                c.physical_at_zero = 3.0;
                c.physical_at_one = 3.0;
            },
            EmptyDomain,
        ),
        (
            |c| {
                c.mapping = ParameterMapping::Linear;
                c.physical_at_zero = f64::NEG_INFINITY;
            },
            NonFiniteBound,
        ),
    ];
    for (break_it, reason) in cases {
        let mut broken = contract("release", 1);
        break_it(&mut broken);
        assert_eq!(broken.validate(), Err(reason), "{broken:?}");
        assert_eq!(
            Topology::new(OTHER, vec![contract("attack", 0), broken]),
            Err(TopologyError::InvalidParameterContract {
                name: OTHER.to_owned(),
                path: "release".to_owned(),
                reason,
            })
        );
    }
    let mut linear_from_zero = contract("sustain", 0);
    linear_from_zero.mapping = ParameterMapping::Linear;
    linear_from_zero.physical_at_zero = 0.0;
    assert_eq!(linear_from_zero.validate(), Ok(()));
}

/// § 2 invariant 4: a contract whose ends are finite but whose linear width `high - low` or
/// exponential ratio `high / low` overflows an f64 would map some `p` to a non-finite value, so it
/// is refused.
#[test]
fn a_range_whose_width_or_ratio_overflows_is_refused() {
    let mut wide = contract("wide", 0);
    wide.mapping = ParameterMapping::Linear;
    wide.physical_at_zero = -1e308;
    wide.physical_at_one = 1e308;
    let mut steep = contract("steep", 0);
    steep.physical_at_zero = f64::from_bits(1);
    steep.physical_at_one = 1.0;
    for broken in [wide, steep] {
        assert_eq!(
            broken.validate(),
            Err(InvalidContractReason::NonFiniteSpan),
            "{broken:?}"
        );
        let path = broken.path.clone();
        assert_eq!(
            Topology::new(OTHER, vec![broken]),
            Err(TopologyError::InvalidParameterContract {
                name: OTHER.to_owned(),
                path,
                reason: InvalidContractReason::NonFiniteSpan,
            })
        );
    }
    let mut widest_finite = contract("widest_finite", 0);
    widest_finite.mapping = ParameterMapping::Linear;
    widest_finite.physical_at_zero = -f64::MAX / 2.0;
    widest_finite.physical_at_one = f64::MAX / 2.0;
    assert_eq!(widest_finite.validate(), Ok(()));
    for p in [0.0, 0.5, 1.0] {
        assert!(widest_finite.to_physical(p).is_finite());
    }
}

#[test]
fn a_repeated_path_or_index_is_refused() {
    assert_eq!(
        Topology::new(OTHER, vec![contract("decay", 0), contract("decay", 1)]),
        Err(TopologyError::DuplicateParameterPath {
            name: OTHER.to_owned(),
            path: "decay".to_owned()
        })
    );
    assert_eq!(
        Topology::new(OTHER, vec![contract("decay", 1), contract("release", 1)]),
        Err(TopologyError::DuplicateParameterIndex {
            name: OTHER.to_owned(),
            index: 1
        })
    );
    assert!(Topology::new(OTHER, vec![contract("decay", 1), contract("release", 0)]).is_ok());
}

/// § 5.2: an index is the position in the normalized target vector, so the indices of n contracts
/// are exactly 0..n-1 (coordinator decision, wave a2 correction 1). Distinct indices that leave a
/// gap put one of them at n or above, which is refused as an invalid contract naming it.
#[test]
fn the_indices_are_exactly_zero_to_n_minus_one() {
    for (indices, outside) in [(vec![1], "p1"), (vec![0, 2], "p2"), (vec![3, 0, 1], "p3")] {
        let contracts = indices
            .iter()
            .map(|&i| contract(&format!("p{i}"), i))
            .collect();
        assert_eq!(
            Topology::new(OTHER, contracts),
            Err(TopologyError::InvalidParameterContract {
                name: OTHER.to_owned(),
                path: outside.to_owned(),
                reason: InvalidContractReason::IndexOutsideTargetVector,
            }),
            "{indices:?}"
        );
    }
    let shuffled = [2, 0, 1]
        .iter()
        .map(|&i| contract(&format!("p{i}"), i))
        .collect();
    assert!(Topology::new(OTHER, shuffled).is_ok());
    assert!(Topology::new(OTHER, Vec::new()).is_ok());
}

/// The topology errors carry the topology's name, as `patchback.patch`'s
/// `DuplicateParameterPath`, `DuplicateParameterIndex` and `InvalidParameterContract` do.
#[test]
fn a_topology_refusal_names_the_topology() {
    let mut invalid = contract("attack", 0);
    invalid.default_normalized = 2.0;
    for (contracts, display) in [
        (
            vec![contract("decay", 0), contract("decay", 1)],
            "topology test-voice/1: parameter path decay is declared twice",
        ),
        (
            vec![contract("decay", 0), contract("release", 0)],
            "topology test-voice/1: parameter index 0 is declared twice",
        ),
        (
            vec![invalid],
            "topology test-voice/1: invalid parameter contract attack: the normalized default is not in [0, 1]",
        ),
    ] {
        let error = Topology::new(OTHER, contracts).unwrap_err();
        let name = match &error {
            TopologyError::InvalidParameterContract { name, .. }
            | TopologyError::DuplicateParameterPath { name, .. }
            | TopologyError::DuplicateParameterIndex { name, .. } => name,
        };
        assert_eq!(name, OTHER);
        assert_eq!(error.to_string(), display);
    }
}
