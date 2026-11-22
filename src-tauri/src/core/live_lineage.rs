//! Live-world lineage: births, deaths and unique ids for the live Bevy population (FINDINGS F-066).
//!
//! # The gap this closes
//!
//! Until this module, a birth in the live world never reached a
//! [`LineageTracker`](crate::evolution::lineage::LineageTracker). The only caller of
//! `add_reproduction` was the MAP-Elites epoch on the evolution thread, so the continuous
//! reproduction of ADR-0005 (`autonomous_reproduction_system` → `SpawnOffspringCommand`) left no
//! edge behind. And the id it gave a child was `format!("{parent}.{generation + 1}")`, which is the
//! same string for every child of one parent — the root cause F-047 worked around in inference
//! pairing rather than removed. Line-of-descent measurement on the live population (OSS-073, the
//! S43 Red-Queen gate, AE-S11 species candidates) needs both a graph and ids that can name its
//! nodes.
//!
//! # Opt-in, and why
//!
//! The id is not just a label. `autonomous_reproduction_system` iterates parents **sorted by
//! lineage id** (and the drift-replay candidate order keys on it too), so the order in which
//! parents draw from `SimRng` follows the id strings. Minting a different id therefore changes
//! which parent reproduces first, every later draw, and the world checksum. Calibrated live
//! manifests (E3, SEL2-P2) were measured on the old ids, so the whole path is behind
//! `live.lineage_tracking = 1.0` ([`LiveLineage`] is inserted only then). With the resource absent,
//! every function here is unreachable and the commands that consult it only test for its presence,
//! which neither allocates nor draws — the legacy trajectory is bit-identical and
//! `LIVE_MODEL_VERSION` does not move (ADR-0008 compares checksums only between runs of the same
//! model version, and no existing run changes).
//!
//! # Where the work happens
//!
//! Only in deferred commands — `SpawnOffspringCommand::apply` and
//! `ReclaimAndDespawnAgentCommand::apply` — which already run once per birth or death at a sync
//! point, never in a per-tick system. That is where this module's allocations (the minted id, the
//! genotype the node keeps, the tracker's vectors) are allowed to be. The physics, CPG and
//! collision loops are not touched.
//!
//! # Ids
//!
//! A founder keeps its genesis id (`live-founder-NNNN`). A child gets
//! `birth-{run:016x}-{n:08x}` — the [`RunIdentity`] format, under its own prefix so the
//! MAP-Elites `lineage-…` namespace and its checkpoint cursor never see it. The cursor `n` is saved
//! ([`SavedLiveLineage`]) rather than recovered from the ids in a save, because compaction deletes
//! dead nodes: recovering it from what survived could re-issue an id that already existed, and the
//! sort-order coupling above would turn that into a different trajectory after a restore.
//!
//! # What is not covered
//!
//! - The desktop app does not set the key; its `simulation_loop` tracker still sees MAP-Elites
//!   offspring only. Wiring it is a separate change (it also runs Neo4j, which has no columns for
//!   the ticks yet).
//! - Evolutionary replacement does not run in the adapter (ADR-0005), so its births are not
//!   recorded here; replacement retirement goes through the same despawn command and would be
//!   recorded as a death.
//! - Hibernation is not death: a dormant agent stays an open node.

use crate::core::determinism::RunIdentity;
use crate::evolution::genotype::MorphologyGenotype;
use crate::evolution::lineage::{
    InMemoryLineageTracker, LineageNode, LineageRelation, LineageTracker, RelationType,
};
use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

/// Namespace of the ids minted for live births.
pub const LIVE_BIRTH_ID_PREFIX: &str = "birth";

/// Node count at which the first compaction runs. Later thresholds double from what compaction
/// left, so the cost stays amortised O(1) per birth while memory stays O(alive + branch points).
pub const FIRST_COMPACTION_AT: u64 = 4_096;

/// The part of [`LiveLineage`] that is not the graph itself. The graph travels in
/// `SavedSimulationState::lineage_nodes` / `lineage_relations`, like every other tracker's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedLiveLineage {
    /// Run the ids belong to.
    pub run_id: u64,
    /// Ids already minted. The next is `birth-{run_id}-{ids_issued}`.
    pub ids_issued: u64,
    /// Node count that triggers the next compaction.
    pub next_compaction_at: u64,
}

/// The live population's lineage graph and id cursor. Present only when
/// `live.lineage_tracking = 1.0`.
#[derive(Resource)]
pub struct LiveLineage {
    tracker: InMemoryLineageTracker,
    ids: RunIdentity,
    node_count: u64,
    next_compaction_at: u64,
    /// World tick stamped on births and deaths. The adapter sets it before each tick runs.
    tick: u64,
    /// Writes the tracker refused. A refusal is a bookkeeping bug (unknown parent, second death),
    /// never a normal outcome; it is counted rather than panicking because it cannot change the
    /// trajectory, and the tests assert it stays zero.
    faults: u64,
}

impl LiveLineage {
    /// An empty graph for a fresh run.
    pub fn new(run_id: u64) -> Self {
        Self {
            tracker: InMemoryLineageTracker::new(),
            ids: RunIdentity::new(run_id, LIVE_BIRTH_ID_PREFIX),
            node_count: 0,
            next_compaction_at: FIRST_COMPACTION_AT,
            tick: 0,
            faults: 0,
        }
    }

    /// Rebuild from a save. `saved` is `None` for a save written before this module existed, or
    /// by a run without the key; the cursor is then recovered from every id the save still holds,
    /// which is the best available and cannot collide with any of them.
    pub fn restore(
        run_id: u64,
        saved: Option<&SavedLiveLineage>,
        nodes: Vec<LineageNode>,
        relations: Vec<LineageRelation>,
        agent_ids: impl IntoIterator<Item = String>,
    ) -> Result<Self, String> {
        let recovered = crate::core::determinism::issued_after_existing_ids(
            run_id,
            LIVE_BIRTH_ID_PREFIX,
            nodes
                .iter()
                .map(|n| n.id.clone())
                .chain(
                    relations
                        .iter()
                        .flat_map(|r| [r.source_id.clone(), r.target_id.clone()]),
                )
                .chain(agent_ids),
        )?;
        let (issued, next_compaction_at) = match saved {
            Some(s) => {
                if s.run_id != run_id {
                    return Err(format!(
                        "live lineage cursor belongs to run {:016x}, not {run_id:016x}",
                        s.run_id
                    ));
                }
                if s.ids_issued < recovered {
                    return Err(
                        "live lineage id cursor precedes ids already present in the save".into(),
                    );
                }
                (s.ids_issued, s.next_compaction_at)
            }
            None => (recovered, FIRST_COMPACTION_AT.max(2 * nodes.len() as u64)),
        };
        let node_count = nodes.len() as u64;
        let tracker = InMemoryLineageTracker::new();
        tracker.load_state(nodes, relations);
        Ok(Self {
            tracker,
            ids: RunIdentity::with_issued(run_id, LIVE_BIRTH_ID_PREFIX, issued),
            node_count,
            next_compaction_at,
            tick: 0,
            faults: 0,
        })
    }

    /// What a save must carry besides the graph.
    pub fn saved(&self) -> SavedLiveLineage {
        SavedLiveLineage {
            run_id: self.ids.run_id(),
            ids_issued: self.ids.issued(),
            next_compaction_at: self.next_compaction_at,
        }
    }

    /// The whole graph, cloned.
    pub fn graph(&self) -> (Vec<LineageNode>, Vec<LineageRelation>) {
        self.tracker.get_lineage_graph().unwrap_or_default()
    }

    /// Refused writes so far. Zero on a healthy run.
    pub fn faults(&self) -> u64 {
        self.faults
    }

    /// The tick births and deaths are stamped with until the next call.
    pub fn set_tick(&mut self, tick: u64) {
        self.tick = tick;
    }

    /// A genesis founder: a root, born at tick 0.
    pub fn record_founder(&mut self, id: String, genotype: MorphologyGenotype) {
        let result = self
            .tracker
            .add_root(id.clone(), genotype)
            .and_then(|()| self.tracker.record_origin(&id, 0));
        self.note(result);
        self.node_count += 1;
    }

    /// The id for the next live birth.
    pub fn mint_birth_id(&mut self) -> String {
        match self.ids.try_next_id() {
            Ok(id) => id,
            Err(error) => panic!("live lineage: {error}"),
        }
    }

    /// One live birth: a node, an edge from each parent, and its origin tick.
    pub fn record_birth(
        &mut self,
        id: String,
        generation: u32,
        genotype: MorphologyGenotype,
        parents: Vec<String>,
        relation_type: RelationType,
    ) {
        let result =
            self.tracker
                .add_reproduction(id.clone(), generation, genotype, parents, relation_type);
        // The node is written even when an edge is refused, so stamp its origin either way.
        let origin = self.tracker.record_origin(&id, self.tick);
        self.note(result);
        self.note(origin);
        self.node_count += 1;
        if self.node_count >= self.next_compaction_at {
            self.compact();
        }
    }

    /// An individual left the population at the current tick.
    pub fn record_death(&mut self, id: &str) {
        let result = self.tracker.record_destruction(id, self.tick);
        self.note(result);
    }

    /// Prune extinct branches and splice unary paths, keeping every open individual.
    fn compact(&mut self) {
        match self.tracker.compact(&[]) {
            Ok(report) => {
                self.node_count = report.nodes_after as u64;
                self.next_compaction_at = FIRST_COMPACTION_AT.max(2 * self.node_count);
            }
            Err(error) => {
                eprintln!("live lineage: compaction skipped: {error}");
                self.faults += 1;
                // Do not retry on every birth against a graph that cannot be compacted.
                self.next_compaction_at = self.next_compaction_at.saturating_mul(2);
            }
        }
    }

    fn note(&mut self, result: Result<(), String>) {
        if let Err(error) = result {
            eprintln!("live lineage: {error}");
            self.faults += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minted_ids_use_their_own_namespace_and_count_up() {
        let mut live = LiveLineage::new(0xab);
        assert_eq!(live.mint_birth_id(), "birth-00000000000000ab-00000000");
        assert_eq!(live.mint_birth_id(), "birth-00000000000000ab-00000001");
        assert_eq!(live.saved().ids_issued, 2);
        // The MAP-Elites checkpoint cursor ignores them.
        assert_eq!(
            crate::core::determinism::issued_after_existing_ids(
                0xab,
                "lineage",
                ["birth-00000000000000ab-00000001"]
            ),
            Ok(0)
        );
    }

    #[test]
    fn births_and_deaths_are_stamped_and_a_second_death_is_a_fault() {
        let mut live = LiveLineage::new(1);
        live.record_founder("f".into(), MorphologyGenotype::new());
        live.set_tick(5);
        let child = live.mint_birth_id();
        live.record_birth(
            child.clone(),
            1,
            MorphologyGenotype::new(),
            vec!["f".into()],
            RelationType::Clone,
        );
        live.set_tick(9);
        live.record_death(&child);
        assert_eq!(live.faults(), 0);
        let (nodes, rels) = live.graph();
        let c = nodes.iter().find(|n| n.id == child).unwrap();
        assert_eq!((c.origin_tick, c.destruction_tick), (Some(5), Some(9)));
        assert_eq!(rels.len(), 1);
        live.record_death(&child);
        assert_eq!(live.faults(), 1);
    }

    #[test]
    fn restore_without_a_cursor_continues_after_the_greatest_id_in_the_save() {
        let mut live = LiveLineage::new(7);
        live.record_founder("f".into(), MorphologyGenotype::new());
        for _ in 0..3 {
            let id = live.mint_birth_id();
            live.record_birth(
                id,
                1,
                MorphologyGenotype::new(),
                vec!["f".into()],
                RelationType::Clone,
            );
        }
        let (nodes, rels) = live.graph();
        let mut back = LiveLineage::restore(7, None, nodes, rels, Vec::new()).unwrap();
        assert_eq!(back.mint_birth_id(), "birth-0000000000000007-00000003");

        let behind = SavedLiveLineage {
            run_id: 7,
            ids_issued: 1,
            next_compaction_at: FIRST_COMPACTION_AT,
        };
        let (nodes, rels) = live.graph();
        assert!(LiveLineage::restore(7, Some(&behind), nodes, rels, Vec::new()).is_err());
    }

    #[test]
    fn compaction_keeps_open_individuals_without_being_told() {
        let mut live = LiveLineage::new(3);
        live.record_founder("f".into(), MorphologyGenotype::new());
        let a = live.mint_birth_id();
        let b = live.mint_birth_id();
        for id in [&a, &b] {
            live.record_birth(
                id.clone(),
                1,
                MorphologyGenotype::new(),
                vec!["f".into()],
                RelationType::Clone,
            );
        }
        live.record_death(&b);
        live.compact();
        let (nodes, _) = live.graph();
        let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&a.as_str()), "the living child is kept");
        assert!(ids.contains(&"f"), "its parent is kept");
        assert!(!ids.contains(&b.as_str()), "the dead leaf is pruned");
    }
}
