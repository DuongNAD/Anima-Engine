//! F-040 characterization: a live world restored from its own snapshot is *not* a faithful
//! continuation once agents have died, even though the checksum matches the instant it is restored.
//!
//! FIXED: these tests now assert the restore follows the original (they pinned the defect before).
//!
//! What was measured (seed 1, default genesis, `live.mortality = 1`, serialization and restore
//! through `LiveExperimentAdapter::snapshot` / `from_snapshot`, checkpointed every 25 loop iterations; each checkpoint also steps the original one extra tick,
//! so the labels below are iteration counts, not world ticks):
//!
//! - the checksum immediately after `from_snapshot` equals the original's at every sample;
//! - after ONE more tick the two worlds differ at samples 1850 and 1875 (mortality only), and at 41 of
//!   100 samples with births enabled too (mortality+births, sweep to 2500); with births only
//!   (nobody dies) at 0 of 100;
//! - the JSON diff of the two snapshots at k shows the same data but the two segments of one agent
//!   in the opposite order (the original's segment table rows were permuted by earlier despawns);
//!   one tick later that agent's energy differs by ~0.006 EU.
//!
//! The mechanism (confirmed by the fix: forcing a stable joint order removes every divergence): ECS table row order is history-dependent, a restore
//! rebuilds it from scratch, and an order-dependent system (`resolve_joints_system` iterates its
//! joint query in table order) turns the permutation into a different trajectory.

use anima_engine_lib::core::agent_systems::AgentLineageId;
use anima_engine_lib::core::causal::CausalLedger;
use anima_engine_lib::core::experiment::{InitialConditionSet, WorldLawSet};
use anima_engine_lib::core::experiment_runner::ExperimentModel;
use anima_engine_lib::core::intervention::InterventionCommand;
use anima_engine_lib::core::live_experiment::LiveExperimentAdapter;
use anima_engine_lib::core::resources::ReproductionPolicy;
use anima_engine_lib::core::sim_clock::SimClock;
use rand::SeedableRng;

const SEED: u64 = 1;

fn mortality_world() -> LiveExperimentAdapter {
    let init = InitialConditionSet::new(vec![("live.mortality".to_string(), 1.0)]);
    LiveExperimentAdapter::from_manifest(&WorldLawSet::baseline(), &init, &[], SEED, (16, 16), 3000)
        .expect("build")
}

struct Run {
    clock: SimClock,
    ledger: CausalLedger,
    rng: rand::rngs::StdRng,
}

impl Run {
    fn new() -> Self {
        Self {
            clock: SimClock { tick: 0 },
            ledger: CausalLedger::new(),
            rng: rand::rngs::StdRng::seed_from_u64(SEED),
        }
    }
    fn step(&mut self, a: &mut LiveExperimentAdapter) {
        let none: Vec<&InterventionCommand> = vec![];
        let _ = self.clock.advance();
        a.step(&self.clock, &none, &mut self.ledger, &mut self.rng);
    }
}

/// Checkpoint every 25 ticks; at each, restore a copy, step both one tick, and report the ticks at
/// which the two checksums differ. (Same sampling procedure as the sweep that found F-040.)
fn diverging_samples(a: &mut LiveExperimentAdapter, until: u64, resume_policy: bool) -> Vec<u64> {
    let mut run = Run::new();
    let mut bad = vec![];
    for t in 1..=until {
        run.step(a);
        if t % 25 != 0 {
            continue;
        }
        let mut resumed = LiveExperimentAdapter::from_snapshot(&a.snapshot()).expect("resume");
        if resume_policy {
            resumed
                .world()
                .insert_resource(ReproductionPolicy { enabled: true });
        }
        assert_eq!(
            a.checksum(),
            resumed.checksum(),
            "the restore must reproduce the checksum at the checkpoint (tick {t})"
        );
        let mut resumed_run = Run {
            clock: run.clock,
            ledger: CausalLedger::new(),
            rng: rand::rngs::StdRng::seed_from_u64(SEED),
        };
        resumed_run.step(&mut resumed);
        run.step(a);
        if a.checksum() != resumed.checksum() {
            bad.push(t);
        }
    }
    bad
}

#[test]
fn a_restore_after_deaths_follows_the_original_on_the_next_tick() {
    let mut a = mortality_world();
    let bad = diverging_samples(&mut a, 1900, false);
    println!("F040 mortality-only diverging checkpoints: {bad:?}");
    // F-040 (fixed): before the fix this was [1850, 1875] on macOS arm64 (tick lists are platform-specific; only the empty-list property is asserted). `resolve_joints_system` now visits
    // joints by segment id instead of ECS table order.
    assert!(bad.is_empty(), "restore diverged at {bad:?}");
}

#[test]
fn a_restore_after_deaths_and_births_follows_the_original_on_the_next_tick() {
    // Before the fix 41 of 100 checkpoints diverged with mortality and reproduction both on.
    let mut a = mortality_world();
    a.world()
        .insert_resource(ReproductionPolicy { enabled: true });
    let bad = diverging_samples(&mut a, 2500, true);
    println!("F040 mortality+births diverging checkpoints: {bad:?}");
    assert!(bad.is_empty(), "restore diverged at {bad:?}");
}

#[test]
fn without_deaths_the_same_restore_does_follow_the_original() {
    // Control: births only, nobody can die (mortality off). Same sampling that diverged above.
    let init = InitialConditionSet::new(vec![]);
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &init,
        &[],
        SEED,
        (16, 16),
        3000,
    )
    .expect("build");
    a.world()
        .insert_resource(ReproductionPolicy { enabled: true });
    let bad = diverging_samples(&mut a, 1900, true);
    assert!(bad.is_empty(), "diverged at {bad:?}");
}

#[test]
fn siblings_share_one_lineage_id_which_makes_the_reproduction_sort_key_non_unique() {
    // Secondary smell (agent_systems.rs `child_lineage`): every child of one parent gets
    // "<parent>.<generation + 1>". It is NOT the cause of the divergence above (a total tie-break
    // in a scratch build left the failing trajectory unchanged), but it is a latent ordering hazard.
    let init = InitialConditionSet::new(vec![("live.mortality".to_string(), 1.0)]);
    let mut a = LiveExperimentAdapter::from_manifest(
        &WorldLawSet::baseline(),
        &init,
        &[],
        SEED,
        (16, 16),
        3000,
    )
    .expect("build");
    a.world()
        .insert_resource(ReproductionPolicy { enabled: true });
    let mut run = Run::new();
    let mut found: Option<(u64, String)> = None;
    for t in 1..=1500u64 {
        run.step(&mut a);
        let mut w = a.world();
        let mut q = w.query::<&AgentLineageId>();
        let mut seen = std::collections::BTreeSet::new();
        for l in q.iter(&w) {
            if !seen.insert(l.0.clone()) {
                found = Some((t, l.0.clone()));
                break;
            }
        }
        if found.is_some() {
            break;
        }
    }
    let (tick, id) = found.expect("no two living agents ever shared a lineage id");
    println!("F040 first duplicate lineage id at tick {tick}: {id}");
}
