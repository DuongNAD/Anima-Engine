use super::*;
use crate::core::exotic_energy::ExoticEnergyLaw;
use crate::core::intervention::InterventionKind;

fn ref_init() -> InitialConditionSet {
    InitialConditionSet::new(vec![
        ("precip".into(), 1.0),
        ("temperature".into(), 0.5),
        ("plants".into(), 100.0),
        ("herbivores".into(), 40.0),
        ("predators".into(), 8.0),
        ("detritus".into(), 0.0),
    ])
}

pub(super) fn base_manifest() -> ExperimentManifest {
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: "exp-1".into(),
        name: "baseline".into(),
        observer: ObserverPolicy::default(),
        island: None,
        world_identity: WorldIdentity {
            seed: 1337,
            generator_version: 20,
            width: 128,
            height: 128,
            checksum: 0xDEAD_BEEF,
            sim_world_sha256: None,
            genesis_phase: None,
        },
        laws: WorldLawSet::baseline(),
        initial_conditions: ref_init(),
        interventions: vec![],
        seeds: vec![1, 2, 3],
        duration_ticks: 6000,
        sample_period: 600,
        observable_ids: vec!["plants".into(), "herbivores".into()],
        exotic_interventions: Vec::new(),
    }
}

// ---- AE-308: AE3 observables & the initial-condition seam --------------------------------

use crate::core::evolution_pathway as ae3;

fn ae3_init() -> InitialConditionSet {
    let mut v = ref_init().values;
    v.push((ae3::AE3_KEY_POPULATION_TOTAL.into(), 100.0));
    v.push((ae3::AE3_KEY_POPULATION_CAPACITY.into(), 100.0));
    v.push((ae3::AE3_KEY_PATHWAY_FRACTION.into(), 0.5));
    InitialConditionSet::new(v)
}

#[test]
fn ae308_registry_fully_describes_every_ae3_observable() {
    let reg = ObservableRegistry::reference_default();
    reg.validate().expect("the extended registry stays valid");
    for id in ae3::AE3_OBSERVABLE_IDS {
        let spec = reg
            .get(id)
            .unwrap_or_else(|| panic!("AE3 observable '{id}' has no registry spec"));
        assert!(!spec.unit.is_empty(), "{id} needs a unit");
        assert!(!spec.source.is_empty(), "{id} needs a source symbol");
        assert!(spec.cadence_period > 0, "{id} needs a real cadence");
        assert!(
            spec.valid_min.is_finite() && spec.valid_max.is_finite(),
            "{id} bounds must be JSON-safe"
        );
        assert!(spec.valid_min <= spec.valid_max);
    }
    // A frequency is a share, so its declared range says so rather than being open-ended.
    let freq = reg.get("evolution.pathway_frequency").unwrap();
    assert_eq!((freq.valid_min, freq.valid_max), (0.0, 1.0));
    // A signed delta must admit negative values, or a pathway LOSING ground would be unreportable.
    let delta = reg.get("evolution.performance_delta").unwrap();
    assert!(delta.valid_min < 0.0);
    // The model emits the cumulative births counter. Summing those snapshots over a cadence
    // window would count the same births repeatedly, so its metadata must be instantaneous.
    let births = reg.get("evolution.births").unwrap();
    assert_eq!(births.aggregation, Aggregation::Instant);
    // MU-ledger observables carry the exotic conservation role, never the closed-EU one.
    for id in ["exotic.uptake", "exotic.spent"] {
        assert_eq!(
            reg.get(id).unwrap().conservation,
            ConservationRole::ExoticMu
        );
    }
}

#[test]
fn ae308_manifest_rejects_an_ae3_observable_without_an_enabled_population() {
    let reg = ObservableRegistry::reference_default();
    // Requesting a selection observable from a world that has no population would return a
    // fabricated zero. It must fail preflight instead.
    let mut m = base_manifest();
    m.observable_ids.push("evolution.pathway_frequency".into());
    let err = m.validate(&reg).unwrap_err();
    assert!(
        matches!(err, ExperimentError::InvalidPopulation { .. }),
        "expected InvalidPopulation, got {err:?}"
    );

    // With the population enabled, the same manifest validates.
    let mut ok = m.clone();
    ok.initial_conditions = ae3_init();
    ok.validate(&reg)
        .expect("an enabled population may emit AE3 observables");
}

#[test]
fn ae3_initial_conditions_reject_malformed_and_unknown_keys() {
    let reg = ObservableRegistry::reference_default();
    let with = |pairs: Vec<(String, f64)>| {
        let mut m = base_manifest();
        let mut v = ref_init().values;
        v.extend(pairs);
        m.initial_conditions = InitialConditionSet::new(v);
        m.validate(&reg)
    };

    // No AE3 keys at all: the population is simply disabled (legacy compatibility).
    assert!(with(vec![]).is_ok());

    // An AE3 key with no population total declares an input that can never take effect.
    assert!(with(vec![(ae3::AE3_KEY_PATHWAY_FRACTION.into(), 0.5)]).is_err());

    // A misspelt AE3 key is rejected rather than silently ignored.
    assert!(with(vec![
        (ae3::AE3_KEY_POPULATION_TOTAL.into(), 100.0),
        ("ae3.pathway_fracton".into(), 0.5),
    ])
    .is_err());

    // Structurally impossible enabled states.
    for bad in [
        vec![(ae3::AE3_KEY_POPULATION_TOTAL.into(), -10.0)],
        vec![(ae3::AE3_KEY_POPULATION_TOTAL.into(), 0.0)],
        vec![
            (ae3::AE3_KEY_POPULATION_TOTAL.into(), 100.0),
            (ae3::AE3_KEY_PATHWAY_FRACTION.into(), 1.5),
        ],
        vec![
            (ae3::AE3_KEY_POPULATION_TOTAL.into(), 100.0),
            (ae3::AE3_KEY_POPULATION_CAPACITY.into(), 10.0),
        ],
        vec![
            (ae3::AE3_KEY_POPULATION_TOTAL.into(), 100.0),
            (ae3::AE3_KEY_GENERATION_TICKS.into(), 0.0),
        ],
        vec![
            (ae3::AE3_KEY_POPULATION_TOTAL.into(), 100.0),
            (ae3::AE3_KEY_GENERATION_TICKS.into(), 90.0),
        ],
        vec![
            (ae3::AE3_KEY_POPULATION_TOTAL.into(), 100.0),
            (ae3::AE3_KEY_GENERATION_TICKS.into(), 600.5),
        ],
    ] {
        assert!(
            with(bad.clone()).is_err(),
            "expected structural rejection for {bad:?}"
        );
    }

    // NaN is already rejected by the InitialConditionSet contract itself.
    assert!(with(vec![(ae3::AE3_KEY_POPULATION_TOTAL.into(), f64::NAN)]).is_err());
}

// ---- AE-S03: canonical fingerprint ------------------------------------------------------

#[test]
fn ae_s03_disabled_law_round_trips_and_has_stable_fingerprint() {
    let laws = WorldLawSet::baseline();
    let json = serde_json::to_string(&laws).unwrap();
    let back: WorldLawSet = serde_json::from_str(&json).unwrap();
    assert_eq!(laws, back);
    assert_eq!(laws.fingerprint(), back.fingerprint());
    // Two independently-built baselines share a fingerprint.
    assert_eq!(WorldLawSet::baseline().fingerprint(), laws.fingerprint());
}

#[test]
fn ae_s03_changing_any_material_law_changes_fingerprint() {
    let baseline = WorldLawSet::baseline().fingerprint();
    // Adding an exotic law changes it.
    let with_law = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(100.0, 4));
    assert_ne!(baseline, with_law.fingerprint());
    // Changing a single law parameter changes it.
    let mut law = ExoticEnergyLaw::mana_patchy(100.0, 4);
    law.diffusion_rate += 0.01;
    let changed = WorldLawSet::with_exotic(law);
    assert_ne!(with_law.fingerprint(), changed.fingerprint());
    // Changing the display name (a label) STILL changes the fingerprint here, because the law's
    // display_name is part of its declared identity — but the source id / physical params are the
    // material ones the gate cares about. Confirm a physical change is detected:
    let mut law2 = ExoticEnergyLaw::mana_patchy(100.0, 4);
    law2.max_density += 1.0;
    assert_ne!(
        with_law.fingerprint(),
        WorldLawSet::with_exotic(law2).fingerprint()
    );
}

#[test]
fn ae_s02_reordered_non_semantic_input_has_same_manifest_fingerprint() {
    let mut a = base_manifest();
    a.seeds = vec![3, 1, 2];
    a.observable_ids = vec!["herbivores".into(), "plants".into()];
    let mut b = base_manifest();
    b.seeds = vec![1, 2, 3];
    b.observable_ids = vec!["plants".into(), "herbivores".into()];
    // Same logical manifest, reordered sets → identical fingerprint.
    assert_eq!(a.fingerprint(), b.fingerprint());
    // Reordering initial-condition entries also does not matter.
    let mut c = base_manifest();
    let mut vals = ref_init().values;
    vals.reverse();
    c.initial_conditions = InitialConditionSet::new(vals);
    assert_eq!(c.fingerprint(), b.fingerprint());
}

#[test]
fn ae_s03_manifest_fingerprint_tracks_world_law_change() {
    let baseline = base_manifest();
    let mut treatment = base_manifest();
    treatment.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(100.0, 4));
    assert_ne!(baseline.fingerprint(), treatment.fingerprint());
}

// ---- Validation (structured errors) -----------------------------------------------------

#[test]
fn validator_accepts_a_well_formed_manifest() {
    let reg = ObservableRegistry::reference_default();
    assert!(base_manifest().validate(&reg).is_ok());
}

#[test]
fn manifest_deserialization_rejects_unknown_declared_inputs() {
    let mut json = serde_json::to_value(base_manifest()).expect("manifest serializes");
    json.as_object_mut()
        .expect("manifest is a JSON object")
        .insert(
            "exotic_intervention".into(),
            serde_json::json!([{"kind": "inject", "tick": 1, "amount": 5.0}]),
        );

    let err = serde_json::from_value::<ExperimentManifest>(json)
        .expect_err("a misspelled scientific input must not be ignored");
    assert!(err.to_string().contains("unknown field"), "{err}");
}

#[test]
fn validator_rejects_structured_failures() {
    let reg = ObservableRegistry::reference_default();

    // Duplicate seed.
    let mut m = base_manifest();
    m.seeds = vec![1, 1, 2];
    assert_eq!(
        m.validate(&reg),
        Err(ExperimentError::DuplicateSeed { seed: 1 })
    );

    // Empty seeds.
    let mut m = base_manifest();
    m.seeds = vec![];
    assert_eq!(m.validate(&reg), Err(ExperimentError::EmptySeeds));

    // Unknown observable.
    let mut m = base_manifest();
    m.observable_ids = vec!["does_not_exist".into()];
    assert_eq!(
        m.validate(&reg),
        Err(ExperimentError::UnknownObservable {
            id: "does_not_exist".into()
        })
    );

    // Zero duration.
    let mut m = base_manifest();
    m.duration_ticks = 0;
    assert!(matches!(
        m.validate(&reg),
        Err(ExperimentError::OutOfRange { .. })
    ));

    // Unsupported schema version.
    let mut m = base_manifest();
    m.schema_version = 999;
    assert!(matches!(
        m.validate(&reg),
        Err(ExperimentError::UnsupportedSchemaVersion { .. })
    ));

    // An exotic law with the EU unit is rejected (MU is not EU).
    let mut m = base_manifest();
    let mut law = ExoticEnergyLaw::mana_uniform(50.0);
    law.unit = UnitId::new(EU_UNIT);
    m.laws = WorldLawSet::with_exotic(law);
    assert!(matches!(
        m.validate(&reg),
        Err(ExperimentError::InvalidLaw { .. })
    ));
}

// ---- AE-S08: factor diff ----------------------------------------------------------------

#[test]
fn ae_s08_control_variant_differs_only_in_exotic_law() {
    let mut treatment = base_manifest();
    treatment.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(100.0, 4));
    let control = treatment.control_variant();
    let diffs = FactorDiff::diff_paths(&control, &treatment);
    assert_eq!(diffs, vec!["laws.exotic_energy".to_string()]);
    // The genesis allowlist accepts exactly this.
    assert!(FactorDiff::genesis_exotic()
        .validate(&control, &treatment)
        .is_ok());
}

#[test]
fn ae_s08_undeclared_difference_is_rejected() {
    let mut treatment = base_manifest();
    treatment.laws = WorldLawSet::with_exotic(ExoticEnergyLaw::mana_patchy(100.0, 4));
    let mut control = treatment.control_variant();
    // Sneak in an off-allowlist change: a different seed set.
    control.seeds = vec![9, 9_9];
    let err = FactorDiff::genesis_exotic()
        .validate(&control, &treatment)
        .unwrap_err();
    assert!(matches!(
        err,
        ExperimentError::UndeclaredFactorDifference { .. }
    ));
}

// ---- AE-109: observable registry --------------------------------------------------------

#[test]
fn observable_registry_validates_and_is_unique() {
    let reg = ObservableRegistry::reference_default();
    assert!(reg.validate().is_ok());
    // Ids are unique and non-empty; every conserved EU/MU observable declares its role.
    for spec in reg.specs() {
        assert!(!spec.id.is_empty());
        assert!(!spec.unit.is_empty());
    }
    assert!(reg.contains("plants"));
    assert!(reg.contains("exotic.density_total"));
    assert!(!reg.contains("nope"));
    // Fingerprint is order-independent.
    assert_eq!(
        reg.fingerprint(),
        ObservableRegistry::reference_default().fingerprint()
    );
}

#[test]
fn observable_registry_rejects_duplicate_ids() {
    let mut reg = ObservableRegistry::reference_default();
    let dup = reg.specs()[0].clone();
    reg.specs.push(dup);
    assert!(matches!(
        reg.validate(),
        Err(ExperimentError::DuplicateId { .. })
    ));
}

// ---- DEFECT A: registry ranges must be JSON-safe and strictly validated -----------------

#[test]
fn defect_a_reference_registry_is_json_safe_and_finite() {
    // A registry carrying f64 infinities cannot round-trip through JSON (serde_json emits
    // `null` for non-finite floats), so the "self-describing" result would silently lose its
    // ranges. Every default spec must therefore declare FINITE bounds.
    let reg = ObservableRegistry::reference_default();
    for s in reg.specs() {
        assert!(
            s.valid_min.is_finite() && s.valid_max.is_finite(),
            "observable '{}' has non-finite bounds [{}, {}]",
            s.id,
            s.valid_min,
            s.valid_max
        );
    }
    // And the whole registry survives a JSON round-trip unchanged.
    let json = serde_json::to_string(&reg).expect("registry serializes");
    let back: ObservableRegistry = serde_json::from_str(&json).expect("registry deserializes");
    assert_eq!(reg, back);
    assert_eq!(reg.fingerprint(), back.fingerprint());
}

#[test]
fn defect_a_registry_validation_rejects_malformed_specs() {
    let base = ObservableRegistry::reference_default();
    let mutate = |f: &dyn Fn(&mut ObservableSpec)| {
        let mut reg = base.clone();
        f(&mut reg.specs[0]);
        reg.validate()
    };

    // Non-finite bounds are rejected (they are not JSON-representable).
    assert!(mutate(&|s| s.valid_max = f64::INFINITY).is_err());
    assert!(mutate(&|s| s.valid_min = f64::NEG_INFINITY).is_err());
    assert!(mutate(&|s| s.valid_max = f64::NAN).is_err());
    assert!(mutate(&|s| s.valid_min = f64::NAN).is_err());

    // A zero cadence period never fires — a meaningless sampling cadence.
    assert!(mutate(&|s| s.cadence_period = 0).is_err());

    // Empty descriptive metadata is rejected (id/unit were already covered).
    assert!(mutate(&|s| s.display_name = String::new()).is_err());
    assert!(mutate(&|s| s.cadence_name = String::new()).is_err());
    assert!(mutate(&|s| s.source = String::new()).is_err());

    // The unmodified default registry still validates.
    assert!(base.validate().is_ok());
}

// ---- DEFECT B: manifest-path intervention validation ------------------------------------

fn iv(id: u32, start: u64, duration: u64) -> InterventionCommand {
    InterventionCommand {
        id,
        cause_id: id,
        kind: InterventionKind::RainfallDelta,
        region: Region::Global,
        start_tick: start,
        duration_ticks: duration,
        intensity: 0.3,
        signed_negative: true,
        curve: Curve::Step,
        reversible: true,
    }
}

#[test]
fn defect_b_manifest_rejects_invalid_intervention_values() {
    let reg = ObservableRegistry::reference_default();
    let with = |cmd: InterventionCommand| {
        let mut m = base_manifest();
        m.interventions = vec![cmd];
        m.validate(&reg)
    };

    // Non-finite / negative intensity.
    let mut c = iv(1, 10, 100);
    c.intensity = f64::NAN as f32;
    assert!(with(c).is_err());
    let mut c = iv(1, 10, 100);
    c.intensity = f32::INFINITY;
    assert!(with(c).is_err());
    let mut c = iv(1, 10, 100);
    c.intensity = -0.5;
    assert!(with(c).is_err());

    // Invalid Radius geometry.
    let mut c = iv(1, 10, 100);
    c.region = Region::Radius {
        cx: f32::NAN,
        cy: 0.0,
        r: 1.0,
    };
    assert!(with(c).is_err());
    let mut c = iv(1, 10, 100);
    c.region = Region::Radius {
        cx: 0.0,
        cy: 0.0,
        r: -1.0,
    };
    assert!(with(c).is_err());

    // Inverted Rect bounds.
    let mut c = iv(1, 10, 100);
    c.region = Region::Rect {
        min_x: 5,
        min_y: 0,
        max_x: 2,
        max_y: 4,
    };
    assert!(with(c).is_err());

    // start + effective_duration overflows u64.
    let c = iv(1, u64::MAX - 1, u64::MAX);
    assert!(with(c).is_err());

    // A schedule that can never be active within 1..=duration_ticks (base_manifest runs 6000).
    let c = iv(1, 900_000, 10);
    assert!(with(c).is_err());
    // start_tick 0 with a 1-tick window ends before tick 1 → never active either.
    let c = iv(1, 0, 1);
    assert!(with(c).is_err());

    // A well-formed, genuinely-active intervention passes.
    assert!(with(iv(1, 600, 3000)).is_ok());
}

// ---- AE-210 (M5): committed JSON fixtures ------------------------------------------------

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/experiments")
        .join(name)
}

fn load_fixture(name: &str) -> String {
    let p = fixture_path(name);
    std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("missing AE fixture {}: {e}", p.display()))
}

/// The committed fixtures are exactly what the serializer produces today.
///
/// This replaces a `#[ignore]`d test that *wrote* the fixtures: a writer nobody runs guards
/// nothing, and a schema change could land with the fixtures describing the previous schema
/// while every other AE-210 test still passed — they only ever asserted that the committed
/// bytes parse and validate, which stale bytes do.
///
/// Bytes, not parsed values, so a change to *how* a manifest serializes is caught too. `\r\n`
/// is normalised first because `core.autocrlf=true` is the default on this project's
/// development machine and on `windows-latest`, so the working-tree bytes of a text file are
/// not necessarily the committed bytes — see `.gitattributes` for the same problem solved by
/// pinning, and the reason it is not worth pinning a fixture whose content is compared
/// semantically anyway.
#[test]
fn ae210_fixtures_match_the_serializer() {
    for (name, manifest) in ae210_reference_manifests() {
        // Same two lines `examples/gen_ae_fixtures.rs` writes with. Kept here rather than
        // exported from the library because the file format is a property of the fixture
        // directory, not of a manifest.
        let expected = serde_json::to_string_pretty(&manifest).expect("serialize fixture") + "\n";
        let committed = load_fixture(name);
        assert_eq!(
            committed.replace("\r\n", "\n"),
            expected,
            "fixture {name} no longer matches the serializer. If the schema changed on \
                 purpose, regenerate with `cargo run --example gen_ae_fixtures`."
        );
    }
}

/// The reference set is the three files that are committed — no more, no fewer.
#[test]
fn ae210_reference_manifests_cover_exactly_the_committed_files() {
    use std::collections::BTreeSet;
    let declared: BTreeSet<String> = ae210_reference_manifests()
        .into_iter()
        .map(|(n, _)| n.to_string())
        .collect();
    let on_disk: BTreeSet<String> = std::fs::read_dir(fixture_path(""))
        .expect("fixture dir")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".json"))
        .collect();
    assert_eq!(
        declared, on_disk,
        "the committed fixture files and `ae210_reference_manifests` have drifted apart"
    );
}

#[test]
fn ae210_m5_baseline_and_mana_fixtures_round_trip_and_validate() {
    let reg = ObservableRegistry::reference_default();

    for name in ["baseline-no-exotic.json", "mana-patchy-renewable.json"] {
        let raw = load_fixture(name);
        let m: ExperimentManifest =
            serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{name} parses: {e}"));
        m.validate(&reg)
            .unwrap_or_else(|e| panic!("{name} must be a valid manifest: {e}"));

        // Round-trip is lossless and identity-stable.
        let re = serde_json::to_string(&m).expect("serializes");
        let back: ExperimentManifest = serde_json::from_str(&re).expect("re-parses");
        assert_eq!(back, m, "{name} round-trip changed the manifest");
        assert_eq!(
            back.fingerprint(),
            m.fingerprint(),
            "{name} fingerprint is not round-trip stable"
        );
    }

    // The baseline fixture is genuinely exotic-free; the Mana fixture is genuinely a live
    // treatment carrying a runtime forcing.
    let base: ExperimentManifest =
        serde_json::from_str(&load_fixture("baseline-no-exotic.json")).unwrap();
    assert!(base.laws.exotic_energy.is_none());
    assert!(base.exotic_interventions.is_empty());

    let mana: ExperimentManifest =
        serde_json::from_str(&load_fixture("mana-patchy-renewable.json")).unwrap();
    let law = mana.laws.exotic_energy.as_ref().expect("mana law present");
    assert_eq!(law.display_name, "Mana", "display label only");
    assert_eq!(
        law.unit.as_str(),
        crate::core::exotic_energy::MU_UNIT,
        "MU is not EU"
    );
    assert!(!mana.exotic_interventions.is_empty());

    // The two differ in the exotic regime, and only there.
    let diffs = FactorDiff::diff_paths(&base, &mana);
    for p in &diffs {
        assert!(
            p == "laws.exotic_energy" || p == "exotic_interventions",
            "fixtures differ at an undeclared path: {p}"
        );
    }
}

#[test]
fn ae210_m5_invalid_fixture_is_rejected_with_a_structured_error() {
    let reg = ObservableRegistry::reference_default();
    let raw = load_fixture("invalid-negative-source.json");
    let m: ExperimentManifest = serde_json::from_str(&raw).expect("parses as JSON");
    // It parses, but must FAIL validation with a structured (not stringly) error.
    let err = m
        .validate(&reg)
        .expect_err("invalid fixture must be rejected");
    assert!(
        matches!(err, ExperimentError::InvalidLaw { .. }),
        "expected a structured InvalidLaw, got {err:?}"
    );
}

#[test]
fn ae210_m5_fixtures_are_small_enough_to_stay_reviewable() {
    // Schema/size record: these are hand-reviewable fixtures, not blobs. If a change balloons
    // one, that is a schema smell worth noticing in review.
    for name in [
        "baseline-no-exotic.json",
        "mana-patchy-renewable.json",
        "invalid-negative-source.json",
    ] {
        let bytes = load_fixture(name).len();
        assert!(
            bytes < 8_192,
            "fixture {name} is {bytes} bytes — too large to review by eye"
        );
    }
}

#[test]
fn intervention_kind_tags_are_distinct() {
    // Guard: the canonical encoding must give every intervention kind a distinct tag.
    use std::collections::HashSet;
    let kinds = [
        InterventionKind::RainfallDelta,
        InterventionKind::TemperatureDelta,
        InterventionKind::Deforest,
        InterventionKind::RemovePredators,
        InterventionKind::AddNutrient,
        InterventionKind::GeneticInjection {
            target_species: Some(1),
            trait_modifications: vec![],
            count: 10,
        },
        InterventionKind::ToxicityPlume {
            center_x: 0.0,
            center_y: 0.0,
            radius: 10.0,
            toxicity_level: 1.0,
            decay_rate: 0.1,
        },
        InterventionKind::PathogenTransmission {
            pathogen_id: 1,
            transmission_rate: 0.5,
            mortality_multiplier: 1.2,
        },
    ];
    let mut hashes = HashSet::new();
    for k in &kinds {
        let mut c = Canon::new();
        canonicalize_intervention_kind(&mut c, k);
        hashes.insert(c.hash());
    }
    assert_eq!(hashes.len(), kinds.len());
}

#[test]
fn wv2_3_manifest_fingerprint_is_legacy_identical_when_sha_none_and_sensitive_when_some() {
    use crate::core::experiment::Canon;
    use crate::core::intervention::{Curve, InterventionKind, Region};

    // Deserialize an existing committed fixture (no stored fingerprint constant in this test) and
    // verify that its fingerprint equals the exact pre-WV2-3 5-field canonical encoding computed
    // directly from the manifest's fields.
    let raw = load_fixture("baseline-no-exotic.json");
    let manifest: ExperimentManifest =
        serde_json::from_str(&raw).expect("parse baseline-no-exotic fixture");
    assert_eq!(manifest.world_identity.sim_world_sha256, None);

    let mut legacy = Canon::new();
    legacy.tag(0xF0).u16(manifest.schema_version);
    legacy
        .tag(0xF1)
        .u32(manifest.world_identity.seed)
        .u32(manifest.world_identity.generator_version)
        .u32(manifest.world_identity.width)
        .u32(manifest.world_identity.height)
        .u32(manifest.world_identity.checksum);
    manifest.laws.canonicalize(&mut legacy);
    manifest.initial_conditions.canonicalize(&mut legacy);

    let mut ivs: Vec<&InterventionCommand> = manifest.interventions.iter().collect();
    ivs.sort_by_key(|cmd| (cmd.start_tick, cmd.id));
    legacy.tag(0xF2).u64(ivs.len() as u64);
    for cmd in ivs {
        legacy.u32(cmd.id).u32(cmd.cause_id);
        // No interventions in baseline-no-exotic; assert it so the legacy encoder never skips.
        assert!(
            matches!(cmd.kind, InterventionKind::RainfallDelta)
                && matches!(cmd.region, Region::Global)
                && matches!(cmd.curve, Curve::Step),
            "unexpected intervention in baseline fixture"
        );
    }

    let mut seeds = manifest.seeds.clone();
    seeds.sort_unstable();
    legacy.tag(0xF3).u64(seeds.len() as u64);
    for s in seeds {
        legacy.u64(s);
    }
    legacy
        .tag(0xF4)
        .u64(manifest.duration_ticks)
        .u64(manifest.sample_period);
    legacy
        .tag(0xF6)
        .u64(manifest.exotic_interventions.len() as u64);
    assert!(manifest.exotic_interventions.is_empty());

    let mut obs = manifest.observable_ids.clone();
    obs.sort();
    legacy.tag(0xF5).u64(obs.len() as u64);
    for id in obs {
        legacy.str(&id);
    }
    legacy.tag(0xF7);
    match manifest.observer {
        ObserverPolicy::Absent => {
            legacy.tag(0x60);
        }
        ObserverPolicy::Spectate => {
            legacy.tag(0x61);
        }
        ObserverPolicy::Inhabit { cause_id } => {
            legacy.tag(0x62).u32(cause_id);
        }
    }

    let legacy_fp = legacy.hash();
    assert_eq!(
        manifest.fingerprint(),
        legacy_fp,
        "sim_world_sha256: None must preserve the exact legacy manifest fingerprint"
    );

    // Mixing a distinct SHA-256 flips the fingerprint and surfaces in FactorDiff.
    let mut m_a = manifest.clone();
    m_a.world_identity.sim_world_sha256 = Some([0x11; 32]);
    let mut m_b = manifest.clone();
    m_b.world_identity.sim_world_sha256 = Some([0x22; 32]);

    assert_ne!(m_a.fingerprint(), legacy_fp);
    assert_ne!(m_b.fingerprint(), legacy_fp);
    assert_ne!(m_a.fingerprint(), m_b.fingerprint());
    assert_eq!(
        FactorDiff::diff_paths(&manifest, &m_a),
        vec!["world_identity".to_string()]
    );
    assert_eq!(
        FactorDiff::diff_paths(&m_a, &m_b),
        vec!["world_identity".to_string()]
    );
}
