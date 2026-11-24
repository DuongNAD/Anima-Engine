//! `run_esel1_experiment --trial`: one seed and one parameter set per invocation, the unit of an
//! exploratory sweep (one OxideSwarm work unit), with a verification file two replicas can compare
//! byte for byte.
//!
//! The runner is `#[path]`-included and driven in-process through `run_with_env`, which takes the
//! value of `ANIMA_SIM_SEED` as an argument so no test mutates the shared process environment. The
//! fixtures are tiny manifests generated into a temp dir (8 founders, 100 ticks, a sample every 10
//! ticks), except where a test names the committed fixtures on purpose.

use anima_engine_lib::core::experiment::{
    ExperimentManifest, InitialConditionSet, WorldLawSet, MANIFEST_SCHEMA_VERSION,
};
use anima_engine_lib::core::live_experiment::{
    LIVE_EXTENDED_OBSERVABLE_IDS, LIVE_KEY_MUTATION_RATE, LIVE_OBSERVABLE_IDS,
};
use anima_engine_lib::core::sha256::sha256_file;
use anima_engine_lib::core::world_artifact::WorldIdentity;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[path = "../examples/run_esel1_experiment.rs"]
mod runner;

const SMOKE_SEED: u64 = 999_971;
const EXP_SEEDS: [u64; 2] = [11, 12];
const TEMPLATE_TICKS: u64 = 100;
const SEED: &str = "4242";
const OTHER_SEED: &str = "4243";

// ---- Fixtures ----------------------------------------------------------------------------------

fn manifest(id: &str, food: f64, mutation: Option<f64>) -> ExperimentManifest {
    let mut values = vec![
        ("live.mortality".to_string(), 1.0),
        ("live.reproduction".to_string(), 1.0),
        ("live.predator_fraction".to_string(), 0.0),
        ("live.founders".to_string(), 8.0),
        ("live.food_cap".to_string(), food),
    ];
    if let Some(m) = mutation {
        values.push((LIVE_KEY_MUTATION_RATE.to_string(), m));
    }
    ExperimentManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        experiment_id: id.to_string(),
        name: id.to_string(),
        world_identity: WorldIdentity::default(),
        laws: WorldLawSet::baseline(),
        initial_conditions: InitialConditionSet::new(values),
        interventions: vec![],
        seeds: vec![SMOKE_SEED],
        duration_ticks: TEMPLATE_TICKS,
        sample_period: 10,
        // Every observable, body mass included: the trial must restrict them itself.
        observable_ids: LIVE_OBSERVABLE_IDS
            .iter()
            .chain(LIVE_EXTENDED_OBSERVABLE_IDS.iter())
            .map(|s| s.to_string())
            .collect(),
        exotic_interventions: Vec::new(),
        observer: Default::default(),
        island: None,
    }
}

/// A plan whose smoke manifests are the trial's template: control food 60, treatment and drift 20.
fn fixture(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("esel1-trial-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, m) in [
        ("smoke-control.json", manifest("t::sc", 60.0, None)),
        ("smoke-treatment.json", manifest("t::st", 20.0, None)),
        ("smoke-drift.json", manifest("t::sd", 20.0, Some(0.0))),
    ] {
        std::fs::write(dir.join(name), serde_json::to_string_pretty(&m).unwrap()).unwrap();
    }
    let prereg = serde_json::json!({
        "seeds": { "execution_order": EXP_SEEDS, "smoke_seed": SMOKE_SEED,
                   "min_complete_pairs_for_a_decision": 2 },
        "duration": { "duration_ticks_ladder": [100, 60, 40] },
        "cost": { "max_wall_clock_minutes": 90 },
        "manifests": {
            "control": "control.json", "treatment": "treatment.json",
            "drift_control_optional_requires_P4": null,
            "smoke_control": "smoke-control.json", "smoke_treatment": "smoke-treatment.json",
            "smoke_drift": "smoke-drift.json"
        }
    });
    std::fs::write(dir.join("esel1-preregistration.json"), prereg.to_string()).unwrap();
    dir
}

fn argv(parts: &[&str], dir: Option<&Path>, out: &Path) -> Vec<String> {
    let mut v: Vec<String> = parts.iter().map(|s| s.to_string()).collect();
    if let Some(d) = dir {
        v.push("--manifest-dir".into());
        v.push(d.display().to_string());
    }
    v.push("--out".into());
    v.push(out.display().to_string());
    v
}

/// Run `--trial <parts>` against `dir` (or the built-in manifests) with no `ANIMA_SIM_SEED`.
fn trial(parts: &[&str], dir: Option<&Path>, out: &Path) -> Result<(), String> {
    let mut all = vec!["--trial"];
    all.extend_from_slice(parts);
    runner::run_with_env(argv(&all, dir, out), None)
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

fn verify_file(out: &Path) -> String {
    read(&out.join("trial").join("verify_checksums.txt"))
}

/// `arm -> final_checksum` from a verify file.
fn checksums(verify: &str) -> std::collections::BTreeMap<String, u32> {
    verify
        .lines()
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            assert_eq!(f.len(), 3, "line '{l}' is not seed,arm,final_checksum");
            (f[1].to_string(), f[2].parse().expect("checksum is a u32"))
        })
        .collect()
}

fn provenance(out: &Path) -> serde_json::Value {
    serde_json::from_str(&read(&out.join("trial").join("provenance.json"))).unwrap()
}

/// The value of `key` in one arm's `initial_conditions_by_arm` entry of provenance.json.
fn ic(prov: &serde_json::Value, arm: &str, key: &str) -> f64 {
    prov["initial_conditions_by_arm"][arm]
        .as_array()
        .unwrap()
        .iter()
        .find(|kv| kv[0] == key)
        .unwrap_or_else(|| panic!("{arm} has no {key}"))[1]
        .as_f64()
        .unwrap()
}

const PARAMS: [&str; 10] = [
    "--seed",
    SEED,
    "--founders",
    "6",
    "--food-cap-abundant",
    "40",
    "--food-cap-scarce",
    "10",
    "--duration-ticks",
    "120",
];

/// `PARAMS` with one flag's value replaced.
fn params_with(flag: &str, value: &'static str) -> Vec<&'static str> {
    let mut p = PARAMS.to_vec();
    let i = p.iter().position(|x| *x == flag).expect("flag in PARAMS");
    p[i + 1] = value;
    p
}

// ---- Determinism of the verification output ----------------------------------------------------

#[test]
fn two_trials_with_the_same_seed_and_parameters_write_identical_verification_files() {
    let dir = fixture("same");
    let (a, b) = (dir.join("out-a"), dir.join("out-b"));
    trial(&PARAMS, Some(&dir), &a).expect("first trial runs");
    trial(&PARAMS, Some(&dir), &b).expect("second trial runs");

    let va = verify_file(&a);
    assert_eq!(
        va.as_bytes(),
        verify_file(&b).as_bytes(),
        "same seed and parameters must give the same bytes"
    );
    // The shape the OxideSwarm sheet compares: sorted LF lines seed,arm,final_checksum, one per arm.
    assert!(!va.contains('\r'), "LF only");
    assert!(va.ends_with('\n'));
    let lines: Vec<&str> = va.lines().collect();
    let mut sorted = lines.clone();
    sorted.sort();
    assert_eq!(lines, sorted, "lines are sorted");
    assert_eq!(
        checksums(&va).keys().cloned().collect::<Vec<_>>(),
        ["control", "drift", "treatment"]
    );
    assert!(lines.iter().all(|l| l.starts_with("4242,")), "{va}");

    // Every file but timing.json is deterministic too, and checksums.sha256 says so.
    for name in [
        "provenance.json",
        "runs.csv",
        "trial-series.csv",
        "checksums.sha256",
    ] {
        assert_eq!(
            read(&a.join("trial").join(name)),
            read(&b.join("trial").join(name)),
            "{name} differs between two identical trials"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_different_seed_changes_the_verification_file() {
    let dir = fixture("seed");
    let (a, b) = (dir.join("out-a"), dir.join("out-b"));
    trial(&PARAMS, Some(&dir), &a).expect("seed 4242 runs");
    trial(&params_with("--seed", OTHER_SEED), Some(&dir), &b).expect("seed 4243 runs");
    let (va, vb) = (verify_file(&a), verify_file(&b));
    assert_ne!(
        va, vb,
        "a different seed left the verification file unchanged"
    );
    let (ca, cb) = (checksums(&va), checksums(&vb));
    for arm in ["control", "treatment", "drift"] {
        assert_ne!(ca[arm], cb[arm], "{arm}: the seed does not reach the world");
    }
    assert!(vb.lines().all(|l| l.starts_with("4243,")), "{vb}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `ANIMA_SIM_SEED` stands in for `--seed`, gives the same bytes, and may not contradict it.
#[test]
fn anima_sim_seed_is_the_seed_when_the_flag_is_absent_and_must_agree_when_both_are_set() {
    let dir = fixture("env");
    let (a, b) = (dir.join("out-flag"), dir.join("out-env"));
    trial(&PARAMS, Some(&dir), &a).expect("--seed runs");
    let without_seed: Vec<&str> = PARAMS[2..].to_vec();
    let mut parts = vec!["--trial"];
    parts.extend_from_slice(&without_seed);
    runner::run_with_env(argv(&parts, Some(&dir), &b), Some(SEED.to_string()))
        .expect("ANIMA_SIM_SEED runs");
    assert_eq!(verify_file(&a), verify_file(&b));
    assert_eq!(
        provenance(&b)["parameter_sources"]["seed"],
        "ANIMA_SIM_SEED"
    );

    let mut both = vec!["--trial"];
    both.extend_from_slice(&PARAMS);
    let err = runner::run_with_env(
        argv(&both, Some(&dir), &dir.join("out-clash")),
        Some(OTHER_SEED.to_string()),
    )
    .expect_err("a seed flag that contradicts ANIMA_SIM_SEED must be refused");
    assert!(err.contains("disagrees"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_seed_resolution_rules() {
    use runner::resolve_trial_seed as r;
    assert_eq!(r(Some(5), None).unwrap().0, 5);
    assert_eq!(r(None, Some("7")).unwrap(), (7, "ANIMA_SIM_SEED"));
    assert_eq!(r(None, Some(" 7 ")).unwrap().0, 7);
    assert_eq!(r(Some(7), Some("7")).unwrap().0, 7);
    assert!(r(Some(7), Some("8")).is_err(), "disagreement is refused");
    assert!(r(None, None).is_err(), "a trial has no default seed");
    assert!(
        r(None, Some("")).is_err(),
        "an empty variable is unset, so no seed"
    );
    assert_eq!(r(Some(3), Some("")).unwrap(), (3, "--seed"));
    for bad in ["x", "-1", "1.5"] {
        let e = r(Some(1), Some(bad)).expect_err(bad);
        assert!(e.contains("ANIMA_SIM_SEED"), "{e}");
    }
}

// ---- The parameters reach the world ------------------------------------------------------------

#[test]
fn the_parameters_override_the_template_and_reach_the_world() {
    let dir = fixture("params");
    let base = dir.join("out-base");
    trial(&PARAMS, Some(&dir), &base).expect("trial runs");
    let prov = provenance(&base);
    for arm in ["control", "treatment", "drift"] {
        assert_eq!(ic(&prov, arm, "live.founders"), 6.0, "{arm}");
    }
    assert_eq!(ic(&prov, "control", "live.food_cap"), 40.0);
    assert_eq!(ic(&prov, "treatment", "live.food_cap"), 10.0);
    assert_eq!(ic(&prov, "drift", "live.food_cap"), 10.0);
    assert_eq!(ic(&prov, "drift", LIVE_KEY_MUTATION_RATE), 0.0);
    assert_eq!(prov["parameters"]["duration_ticks"], 120);
    for k in [
        "founders",
        "food_cap_abundant",
        "food_cap_scarce",
        "duration_ticks",
    ] {
        assert_eq!(prov["parameter_sources"][k], "flag", "{k}");
    }
    // The last sample is at the overridden duration, not the template's 100.
    let series = read(&base.join("trial").join("trial-series.csv"));
    assert!(
        series.lines().any(|l| l.starts_with("4242,control,120,")),
        "{series}"
    );

    let base_ck = checksums(&verify_file(&base));
    let founders = dir.join("out-founders");
    trial(&params_with("--founders", "7"), Some(&dir), &founders).expect("trial runs");
    let ck = checksums(&verify_file(&founders));
    for arm in ["control", "treatment", "drift"] {
        assert_ne!(
            ck[arm], base_ck[arm],
            "{arm}: --founders does not reach the world"
        );
    }
    // Each food cap reaches its own arms and no other: the manipulation check of E-SEL1 amendment 1.
    let abundant = dir.join("out-abundant");
    trial(
        &params_with("--food-cap-abundant", "30"),
        Some(&dir),
        &abundant,
    )
    .expect("trial runs");
    let ck = checksums(&verify_file(&abundant));
    assert_ne!(
        ck["control"], base_ck["control"],
        "--food-cap-abundant does not reach control"
    );
    assert_eq!(
        ck["treatment"], base_ck["treatment"],
        "--food-cap-abundant leaked into treatment"
    );
    assert_eq!(
        ck["drift"], base_ck["drift"],
        "--food-cap-abundant leaked into drift"
    );
    let scarce = dir.join("out-scarce");
    trial(&params_with("--food-cap-scarce", "15"), Some(&dir), &scarce).expect("trial runs");
    let ck = checksums(&verify_file(&scarce));
    assert_eq!(
        ck["control"], base_ck["control"],
        "--food-cap-scarce leaked into control"
    );
    assert_ne!(
        ck["treatment"], base_ck["treatment"],
        "--food-cap-scarce does not reach treatment"
    );
    assert_ne!(
        ck["drift"], base_ck["drift"],
        "--food-cap-scarce does not reach drift"
    );

    let duration = dir.join("out-duration");
    trial(
        &params_with("--duration-ticks", "110"),
        Some(&dir),
        &duration,
    )
    .expect("trial runs");
    assert_ne!(
        checksums(&verify_file(&duration))["control"],
        base_ck["control"],
        "--duration-ticks does not reach the world"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A flag left out keeps the template's value: the smoke manifests are the defaults.
#[test]
fn a_flag_left_out_keeps_the_smoke_manifest_value() {
    let dir = fixture("defaults");
    let out = dir.join("out");
    trial(
        &["--seed", SEED, "--duration-ticks", "30"],
        Some(&dir),
        &out,
    )
    .expect("trial runs");
    let prov = provenance(&out);
    assert_eq!(ic(&prov, "control", "live.founders"), 8.0);
    assert_eq!(ic(&prov, "control", "live.food_cap"), 60.0);
    assert_eq!(ic(&prov, "treatment", "live.food_cap"), 20.0);
    assert_eq!(prov["parameter_sources"]["founders"], "smoke manifest");
    assert_eq!(prov["parameter_sources"]["duration_ticks"], "flag");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The trial runs through the smoke's own code: at the smoke's seed and parameters it ends on the
/// smoke's checksums, arm by arm.
#[test]
fn a_trial_at_the_smoke_seed_and_parameters_ends_on_the_smoke_checksums() {
    let dir = fixture("smokeparity");
    let (smoke_out, trial_out) = (dir.join("out-smoke"), dir.join("out-trial"));
    runner::run_with_env(argv(&["--smoke"], Some(&dir), &smoke_out), None).expect("smoke runs");
    trial(&["--seed", "999971"], Some(&dir), &trial_out).expect("trial runs");

    let runs = read(&smoke_out.join("smoke").join("runs.csv"));
    let header = runs.lines().nth(1).unwrap();
    let cols: Vec<&str> = header.split(',').collect();
    let (arm_i, ck_i) = (
        cols.iter().position(|c| *c == "arm").unwrap(),
        cols.iter().position(|c| *c == "final_checksum").unwrap(),
    );
    let smoke: std::collections::BTreeMap<String, u32> = runs
        .lines()
        .skip(2)
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            (f[arm_i].to_string(), f[ck_i].parse().unwrap())
        })
        .collect();
    assert_eq!(smoke.len(), 3);
    assert_eq!(checksums(&verify_file(&trial_out)), smoke);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- What a trial writes ------------------------------------------------------------------------

#[test]
fn a_trial_writes_exactly_its_files_and_no_body_mass() {
    let dir = fixture("files");
    let out = dir.join("out");
    trial(&PARAMS, Some(&dir), &out).expect("trial runs");
    let trial_dir = out.join("trial");
    let names: BTreeSet<String> = std::fs::read_dir(&trial_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    let expected: BTreeSet<String> = [
        "checksums.sha256",
        "provenance.json",
        "runs.csv",
        "timing.json",
        "trial-series.csv",
        "verify_checksums.txt",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(names, expected);
    for name in &names {
        let text = read(&trial_dir.join(name));
        assert!(
            !text.contains("trait_mass"),
            "{name} names a body-mass observable"
        );
    }
    // Nothing outside trial/.
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 1);

    // checksums.sha256 covers every file but timing.json, and its digests are right.
    let sums = read(&trial_dir.join("checksums.sha256"));
    let covered: Vec<&str> = sums
        .lines()
        .map(|l| l.split_once("  ").unwrap().1)
        .collect();
    assert_eq!(
        covered,
        [
            "provenance.json",
            "runs.csv",
            "trial-series.csv",
            "verify_checksums.txt"
        ]
    );
    for line in sums.lines() {
        let (digest, name) = line.split_once("  ").unwrap();
        assert_eq!(
            sha256_file(&trial_dir.join(name)).unwrap(),
            digest,
            "{name}"
        );
    }
    // The wall clock lives in timing.json only.
    let timing: serde_json::Value =
        serde_json::from_str(&read(&trial_dir.join("timing.json"))).unwrap();
    assert_eq!(timing["runs"].as_array().unwrap().len(), 3);
    assert!(timing["runs"][0]["us_per_tick"].is_number());
    assert!(!read(&trial_dir.join("runs.csv")).contains("us_per_tick"));
    assert!(!read(&trial_dir.join("provenance.json")).contains("utc"));
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- Refusals ----------------------------------------------------------------------------------

#[test]
fn a_trial_refuses_a_preregistered_confirmation_seed() {
    let dir = fixture("reserved");
    for seed in EXP_SEEDS {
        let seed_s = seed.to_string();
        let mut p = PARAMS.to_vec();
        p[1] = &seed_s;
        let err = trial(&p, Some(&dir), &dir.join("out"))
            .expect_err(&format!("seed {seed} is in the execution order"));
        assert!(err.contains("execution order"), "{seed}: {err}");
        assert!(
            !dir.join("out").exists(),
            "nothing is written before a refusal (seed {seed})"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_trial_output_files_contain_no_trait_mass_substring() {
    let dir = fixture("blinding_substring");
    let out = dir.join("out");
    trial(
        &["--seed", SEED, "--duration-ticks", "20"],
        Some(&dir),
        &out,
    )
    .expect("trial runs");
    let trial_dir = out.join("trial");
    let expected = [
        "checksums.sha256",
        "provenance.json",
        "runs.csv",
        "timing.json",
        "trial-series.csv",
        "verify_checksums.txt",
    ];
    for name in expected {
        let path = trial_dir.join(name);
        assert!(
            path.is_file(),
            "missing trial output file {}",
            path.display()
        );
        let text = read(&path);
        assert!(
            !text.contains("trait_mass") && !text.to_ascii_lowercase().contains("trait_mass"),
            "{name} leaked a trait_mass substring"
        );
    }
    let prov = provenance(&out);
    assert_eq!(prov["terrain_source"], "generated");
    assert_ne!(
        prov["world_identity"]["checksum"].as_u64().unwrap_or(0),
        0,
        "provenance.json must record observed non-zero world_identity"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn malformed_or_misplaced_trial_flags_are_refused() {
    let dir = fixture("flags");
    let out = dir.join("out");
    let refused = |parts: &[&str], what: &str| {
        let e = runner::run_with_env(argv(parts, Some(&dir), &out), None)
            .expect_err(&format!("{what} must be refused"));
        assert!(!e.is_empty());
        e
    };
    // The trial flags belong to --trial only; the preregistered modes stay exactly as they were.
    let e = refused(&["--smoke", "--seed", "5"], "--seed with --smoke");
    assert!(e.contains("only meaningful with --trial"), "{e}");
    refused(
        &["--ensemble", "--founders", "5"],
        "--founders with --ensemble",
    );
    refused(
        &["--trial", "--seed", "5", "--duration-rung", "100"],
        "--duration-rung with --trial",
    );
    refused(
        &["--trial", "--seed", "5", "--seed", "6"],
        "a repeated --seed",
    );
    refused(&["--trial", "--seed", "-5"], "a negative seed");
    refused(
        &["--trial", "--seed", "5", "--founders", "2.5"],
        "fractional founders",
    );
    refused(
        &["--trial", "--seed", "5", "--duration-ticks", "0"],
        "a zero duration",
    );
    let e = refused(&["--trial", "--founders", "5"], "a trial without a seed");
    assert!(e.contains("needs a seed"), "{e}");
    let e = refused(
        &[
            "--trial",
            "--seed",
            "5",
            "--food-cap-abundant",
            "30",
            "--food-cap-scarce",
            "30",
        ],
        "equal food caps",
    );
    assert!(e.contains("same world"), "{e}");
    refused(
        &["--trial", "--seed", "5", "--founders", "100000"],
        "founders the live adapter refuses",
    );
    assert!(!out.exists(), "nothing is written before a refusal");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- The built-in manifests ---------------------------------------------------------------------

/// Without `--manifest-dir` the trial reads the committed fixtures compiled into the binary, so a
/// grid task stages nothing but the binary; it runs the same world as pointing at the fixtures.
#[test]
fn the_built_in_manifests_are_the_committed_fixtures() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/experiments_esel1");
    let scratch = std::env::temp_dir().join(format!("esel1-trial-builtin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let (built_in, from_dir) = (scratch.join("out-built-in"), scratch.join("out-dir"));
    let parts = ["--seed", SEED, "--duration-ticks", "15"];
    trial(&parts, None, &built_in).expect("built-in trial runs");
    trial(&parts, Some(&fixtures), &from_dir).expect("trial on the committed fixtures runs");
    assert_eq!(verify_file(&built_in), verify_file(&from_dir));

    let prov = provenance(&built_in);
    assert!(
        prov["manifests"]["source"]
            .as_str()
            .unwrap()
            .starts_with("built-in"),
        "{}",
        prov["manifests"]["source"]
    );
    let hashes = prov["manifests"]["sha256_as_read"].as_object().unwrap();
    assert_eq!(
        hashes.len(),
        4,
        "the preregistration and three smoke manifests"
    );
    for (name, digest) in hashes {
        assert_eq!(
            sha256_file(&fixtures.join(name)).unwrap(),
            digest.as_str().unwrap(),
            "the built-in {name} is not the committed fixture"
        );
    }
    // The committed smoke template: 30 founders, food caps 50 and 25.
    assert_eq!(ic(&prov, "control", "live.founders"), 30.0);
    assert_eq!(ic(&prov, "control", "live.food_cap"), 50.0);
    assert_eq!(ic(&prov, "treatment", "live.food_cap"), 25.0);
    // Every committed preregistration confirmation seed is refused before any output is written.
    let prereg: serde_json::Value =
        serde_json::from_str(&read(&fixtures.join("esel1-preregistration.json"))).unwrap();
    let execution_order: Vec<u64> = prereg["seeds"]["execution_order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    assert_eq!(
        execution_order.len(),
        12,
        "committed E-SEL1 execution order must list all 12 seeds"
    );
    for seed in execution_order {
        let seed_s = seed.to_string();
        let reserved_out = scratch.join(format!("out-reserved-{seed}"));
        let err = trial(&["--seed", &seed_s], None, &reserved_out).expect_err(&format!(
            "{seed} is in E-SEL1's confirmation execution_order"
        ));
        assert!(err.contains("execution order"), "{seed}: {err}");
        assert!(
            !reserved_out.exists(),
            "nothing may be written when seed {seed} is refused"
        );
    }
    let _ = std::fs::remove_dir_all(&scratch);
}
