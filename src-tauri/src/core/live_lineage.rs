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
//! # Recording does not change the trajectory
//!
//! ADR-0008 driver 4 ("Không phản hồi"): recording the past must not change it. A run with
//! `live.lineage_tracking = 1.0` and the same run without it are the same world at every tick —
//! same `world_checksum`, same births and deaths (`recording_lineage_does_not_change_the_trajectory`
//! in `tests/live_lineage_tests.rs` checks it run against run).
//!
//! That is why the unique id is **not** written into `AgentLineageId`. That component is not just a
//! label: `autonomous_reproduction_system` iterates parents sorted by it (and the drift-replay
//! candidate order keys on it too), so the order in which parents draw from `SimRng` follows those
//! strings. The first version of this module overwrote it, and recording then changed which parent
//! reproduced first, every later draw, and the checksum. So `AgentLineageId` and
//! `AgentParentLineageIds` keep their legacy values in every run, and each agent's unique id lives in
//! a side table inside [`LiveLineage`], keyed by root entity (see the type's doc for why not a
//! component). Nothing the schedule reads is touched: no component, no archetype, no `SimRng` draw,
//! no entity id. With the resource absent the commands that consult it only test for its presence,
//! which neither allocates nor draws, and `LIVE_MODEL_VERSION` does not move.
//!
//! The key is still opt-in because recording costs memory and a clone of each child's genotype.
//!
//! # Where the work happens
//!
//! In deferred commands — `SpawnOffspringCommand::apply` and `ReclaimAndDespawnAgentCommand::apply`
//! — which already run once per birth or death at a sync point, and in [`LiveLineage::set_tick`],
//! which the adapter calls between ticks and which runs compaction. Never in a per-tick system. That
//! is where this module's allocations (the minted id, the genotype the node keeps, the tracker's
//! vectors) are allowed to be. The physics, CPG and collision loops are not touched.
//!
//! # Ids
//!
//! A founder keeps its genesis id (`live-founder-NNNN`). A child gets
//! `birth-{run:016x}-{n:08x}` — the [`RunIdentity`] format, under its own prefix so the
//! MAP-Elites `lineage-…` namespace and its checkpoint cursor never see it. The edge from the parent
//! names the parent's own unique id, found through `SpawnOffspringCommand::parent_entity`, not the
//! legacy `parent_ids` string, which siblings share.
//!
//! A save carries each living agent's id in `SerializedAgent::live_id`, and a restore re-attaches it
//! exactly, never re-minting (contract D01). The cursor `n` is saved ([`SavedLiveLineage`]) rather
//! than recovered from the ids in a save, because compaction deletes dead nodes: recovering it from
//! what survived could re-issue an id that already existed.
//!
//! # What is not covered
//!
//! - The desktop app does not set the key; its `simulation_loop` tracker still sees MAP-Elites
//!   offspring only. Wiring it is a separate change (it also runs Neo4j, which has no columns for
//!   the ticks yet).
//! - Evolutionary replacement does not run in the adapter (ADR-0005), so its births are not
//!   recorded here; replacement retirement goes through the same despawn command and would be
//!   recorded as a death.
//! - Hibernation is not death: a dormant agent stays an open node, and its id travels in the
//!   dormant record. The aggregate-population tier (`DehydrateAgentCommand`) does not carry the id;
//!   the adapter never inserts that tier, so it cannot meet this resource today.
//! - Migration between shards is desktop-only and does not carry the id either.

use crate::core::determinism::RunIdentity;
use crate::evolution::genotype::MorphologyGenotype;
use crate::evolution::lineage::{
    InMemoryLineageTracker, LineageNode, LineageRelation, LineageTracker, RelationType,
};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Namespace of the ids minted for live births.
pub const LIVE_BIRTH_ID_PREFIX: &str = "birth";

/// Default node count at which the first compaction runs. Later thresholds double from what
/// compaction left, so the cost stays amortised O(1) per birth while memory stays
/// O(alive + branch points). A run may lower it ([`LiveLineage::set_compaction_floor`]) — the tests
/// do, to compact inside a few thousand ticks — and the value travels in the save.
pub const FIRST_COMPACTION_AT: u64 = 4_096;

fn default_compaction_floor() -> u64 {
    FIRST_COMPACTION_AT
}

/// The part of [`LiveLineage`] that is not the graph itself. The graph travels in
/// `SavedSimulationState::lineage_nodes` / `lineage_relations`, like every other tracker's, and
/// each living agent's id in `SerializedAgent::live_id`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedLiveLineage {
    /// Run the ids belong to.
    pub run_id: u64,
    /// Ids already minted. The next is `birth-{run_id}-{ids_issued}`.
    pub ids_issued: u64,
    /// Node count that triggers the next compaction.
    pub next_compaction_at: u64,
    /// The compaction threshold's floor ([`FIRST_COMPACTION_AT`] unless the run lowered it).
    #[serde(default = "default_compaction_floor")]
    pub compaction_floor: u64,
    /// Writes the tracker refused so far (see [`LiveLineage::faults`]). Saved so a resumed run
    /// reports the whole run's count, not only what happened after the restore.
    #[serde(default)]
    pub faults: u64,
}

/// One agent a restore has just spawned, as [`LiveLineage::restore`] needs to see it.
pub struct RestoredAgent<'a> {
    /// The root entity the restore gave it.
    pub entity: Entity,
    /// What the save says about it.
    pub saved: &'a crate::core::simulation_state::SerializedAgent,
}

/// The live population's lineage graph, id cursor, and the live id of every agent in the world.
/// Present only when `live.lineage_tracking = 1.0`.
///
/// # Why the ids live here and not on the agent
///
/// `AgentLineageId` is not a label the simulation ignores: `autonomous_reproduction_system`
/// iterates parents sorted by it, so the order in which parents draw from `SimRng` follows those
/// strings. Writing a minted id there changed the trajectory (54 births / 26 deaths by tick 2,500
/// with recording on, against 53 / 25 off), which ADR-0008 driver 4 forbids: recording the past
/// must not change it. So `AgentLineageId` keeps the legacy `parent.generation` value in every run,
/// and the unique id lives in this side table, keyed by root [`Entity`].
///
/// A component (`LiveAgentId(String)`) was the other option, and it was rejected: founders exist
/// before the resource does, so it would have to be inserted on them afterwards, moving every
/// founder into a new archetype in whatever order the insertion loop visits them — a different
/// table row order, hence a different iteration order for every query that walks agents. A table
/// the simulation never queries cannot reorder anything. Entities are never reused with the same
/// generation, so a stale key can never name a different agent.
#[derive(Resource)]
pub struct LiveLineage {
    tracker: InMemoryLineageTracker,
    ids: RunIdentity,
    node_count: u64,
    next_compaction_at: u64,
    compaction_floor: u64,
    compactions: u64,
    /// World tick stamped on births and deaths. The adapter sets it before each tick runs.
    tick: u64,
    /// Writes the tracker refused. A refusal is a bookkeeping bug (unknown parent, second death),
    /// never a normal outcome; it is counted rather than panicking because it cannot change the
    /// trajectory, and the tests assert it stays zero.
    faults: u64,
    /// The live id of every agent present in the world, by root entity. Looked up, never iterated
    /// in an order anything depends on.
    live_ids: BTreeMap<Entity, String>,
    /// Agents that died since the current tick began, kept so a birth applied later in the same
    /// command flush can still name a parent that died earlier in it. Cleared by
    /// [`Self::set_tick`].
    departed: Vec<(Entity, String)>,
}

impl LiveLineage {
    /// An empty graph for a fresh run.
    pub fn new(run_id: u64) -> Self {
        Self {
            tracker: InMemoryLineageTracker::new(),
            ids: RunIdentity::new(run_id, LIVE_BIRTH_ID_PREFIX),
            node_count: 0,
            next_compaction_at: FIRST_COMPACTION_AT,
            compaction_floor: FIRST_COMPACTION_AT,
            compactions: 0,
            tick: 0,
            faults: 0,
            live_ids: BTreeMap::new(),
            departed: Vec::new(),
        }
    }

    /// Rebuild from a save, and re-attach every restored agent to the id the save gave it.
    ///
    /// `saved` is `None` for a save written before this module existed, or by a run without the
    /// key; the cursor is then recovered from every id the save still holds, which is the best
    /// available and cannot collide with any of them.
    ///
    /// An agent's saved `live_id` is carried exactly, never re-minted (contract D01). An agent
    /// without one comes from a save that predates it, so there is nothing to carry: it keeps its
    /// `lineage_id` when that names a graph node no other agent has claimed (a founder), and
    /// otherwise is entered as a new root, born at `tick`, under a freshly minted id — its ancestry
    /// before the restore is unknown, and the graph says so rather than guessing a parent.
    pub fn restore(
        run_id: u64,
        saved: Option<&SavedLiveLineage>,
        nodes: Vec<LineageNode>,
        relations: Vec<LineageRelation>,
        agents: &[RestoredAgent<'_>],
        dormant: &[crate::core::agent_systems::DormantAgentRecord],
        tick: u64,
    ) -> Result<Self, String> {
        let recovered = crate::core::determinism::issued_after_existing_ids(
            run_id,
            LIVE_BIRTH_ID_PREFIX,
            nodes
                .iter()
                .map(|n| n.id.as_str())
                .chain(
                    relations
                        .iter()
                        .flat_map(|r| [r.source_id.as_str(), r.target_id.as_str()]),
                )
                .chain(agents.iter().flat_map(|a| {
                    [
                        Some(a.saved.lineage_id.as_str()),
                        a.saved.live_id.as_deref(),
                    ]
                    .into_iter()
                    .flatten()
                }))
                .chain(dormant.iter().flat_map(|d| {
                    [
                        Some(d.agent.lineage_id.as_str()),
                        d.agent.live_id.as_deref(),
                    ]
                    .into_iter()
                    .flatten()
                })),
        )?;
        let (issued, next_compaction_at, compaction_floor, faults) = match saved {
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
                (
                    s.ids_issued,
                    s.next_compaction_at,
                    s.compaction_floor,
                    s.faults,
                )
            }
            None => (
                recovered,
                FIRST_COMPACTION_AT.max(2 * nodes.len() as u64),
                FIRST_COMPACTION_AT,
                0,
            ),
        };
        let node_ids: BTreeSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
        let node_count = nodes.len() as u64;
        let tracker = InMemoryLineageTracker::new();
        tracker.load_state(nodes, relations);
        let mut live = Self {
            tracker,
            ids: RunIdentity::with_issued(run_id, LIVE_BIRTH_ID_PREFIX, issued),
            node_count,
            next_compaction_at,
            compaction_floor,
            compactions: 0,
            tick,
            faults,
            live_ids: BTreeMap::new(),
            departed: Vec::new(),
        };

        // Carried ids first, so a legacy agent cannot adopt a node a carried id already names.
        let mut claimed: BTreeSet<&str> = BTreeSet::new();
        for agent in agents {
            if let Some(id) = agent.saved.live_id.as_deref() {
                if !node_ids.contains(id) {
                    return Err(format!(
                        "restored agent's live id {id:?} has no node in the live lineage graph"
                    ));
                }
                if !claimed.insert(id) {
                    return Err(format!("two restored agents share the live id {id:?}"));
                }
            }
        }
        for d in dormant {
            if let Some(id) = d.agent.live_id.as_deref() {
                if !claimed.insert(id) {
                    return Err(format!("two restored agents share the live id {id:?}"));
                }
            }
        }
        let mut adopted: BTreeSet<String> = BTreeSet::new();
        for agent in agents {
            let id = match &agent.saved.live_id {
                Some(id) => id.clone(),
                None => {
                    let legacy = agent.saved.lineage_id.as_str();
                    if node_ids.contains(legacy)
                        && !claimed.contains(legacy)
                        && !adopted.contains(legacy)
                    {
                        adopted.insert(legacy.to_owned());
                        legacy.to_owned()
                    } else {
                        let id = live.mint_birth_id();
                        let result = live
                            .tracker
                            .add_root(id.clone(), agent.saved.genotype.clone())
                            .and_then(|()| live.tracker.record_origin(&id, tick));
                        live.note(result);
                        live.node_count += 1;
                        id
                    }
                }
            };
            live.live_ids.insert(agent.entity, id);
        }
        Ok(live)
    }

    /// What a save must carry besides the graph and the per-agent ids.
    pub fn saved(&self) -> SavedLiveLineage {
        SavedLiveLineage {
            run_id: self.ids.run_id(),
            ids_issued: self.ids.issued(),
            next_compaction_at: self.next_compaction_at,
            compaction_floor: self.compaction_floor,
            faults: self.faults,
        }
    }

    /// The whole graph, cloned.
    pub fn graph(&self) -> (Vec<LineageNode>, Vec<LineageRelation>) {
        self.tracker.get_lineage_graph().unwrap_or_default()
    }

    /// Nodes in the graph.
    pub fn node_count(&self) -> u64 {
        self.node_count
    }

    /// Node count that triggers the next compaction.
    pub fn next_compaction_at(&self) -> u64 {
        self.next_compaction_at
    }

    /// Compactions this process has run since the resource was built (not saved: a diagnostic).
    pub fn compactions(&self) -> u64 {
        self.compactions
    }

    /// Lower (or raise) the compaction threshold's floor, and re-arm the next compaction from it.
    /// [`FIRST_COMPACTION_AT`] is the default; the value is saved with the run.
    pub fn set_compaction_floor(&mut self, floor: u64) {
        self.compaction_floor = floor.max(1);
        self.next_compaction_at = self.compaction_floor.max(self.node_count + 1);
    }

    /// Refused writes so far. Zero on a healthy run.
    pub fn faults(&self) -> u64 {
        self.faults
    }

    /// The unique live id of the agent rooted at `entity`, if it is in the world.
    pub fn live_id(&self, entity: Entity) -> Option<&str> {
        self.live_ids.get(&entity).map(String::as_str)
    }

    /// The tick births and deaths are stamped with until the next call.
    ///
    /// Called between ticks, so it is also where the previous tick is closed: the agents that died
    /// in it are forgotten (every command that could name them as a parent has been applied), and
    /// the graph is compacted if the births pushed it over the threshold. Compacting here rather
    /// than inside a birth means no compaction can ever run between a parent's death and its
    /// child's birth in one command flush, and prune the node the child is about to name.
    pub fn set_tick(&mut self, tick: u64) {
        self.tick = tick;
        self.departed.clear();
        if self.node_count >= self.next_compaction_at {
            self.compact();
        }
    }

    /// A genesis founder: a root, born at tick 0, whose id is its genesis id.
    pub fn record_founder(&mut self, entity: Entity, id: String, genotype: MorphologyGenotype) {
        let result = self
            .tracker
            .add_root(id.clone(), genotype)
            .and_then(|()| self.tracker.record_origin(&id, 0));
        self.note(result);
        self.node_count += 1;
        self.live_ids.insert(entity, id);
    }

    /// An agent re-entering the world with the id it already had (waking from hibernation). Not a
    /// birth: the node exists and stays as it was.
    pub fn attach(&mut self, entity: Entity, id: String) {
        self.live_ids.insert(entity, id);
    }

    /// An agent leaving the world without dying (hibernation). Its node stays open; the id travels
    /// in the dormant record and comes back through [`Self::attach`].
    pub fn detach(&mut self, entity: Entity) -> Option<String> {
        self.live_ids.remove(&entity)
    }

    fn mint_birth_id(&mut self) -> String {
        match self.ids.try_next_id() {
            Ok(id) => id,
            Err(error) => panic!("live lineage: {error}"),
        }
    }

    /// One live birth: `child` gets a minted id, a node, an edge from `parent`'s id, and its
    /// origin tick. `parent` is looked up among the living and, failing that, among those that died
    /// earlier in this tick's command flush.
    pub fn record_birth(
        &mut self,
        child: Entity,
        parent: Entity,
        generation: u32,
        genotype: MorphologyGenotype,
        relation_type: RelationType,
    ) {
        let id = self.mint_birth_id();
        let parent_id = self.live_ids.get(&parent).cloned().or_else(|| {
            self.departed
                .iter()
                .rev()
                .find(|(e, _)| *e == parent)
                .map(|(_, id)| id.clone())
        });
        let parents = match parent_id {
            Some(p) => vec![p],
            None => {
                eprintln!("live lineage: birth {id} has a parent entity with no live id");
                self.faults += 1;
                Vec::new()
            }
        };
        let result =
            self.tracker
                .add_reproduction(id.clone(), generation, genotype, parents, relation_type);
        // The node is written even when an edge is refused, so stamp its origin either way.
        let origin = self.tracker.record_origin(&id, self.tick);
        self.note(result);
        self.note(origin);
        self.node_count += 1;
        self.live_ids.insert(child, id);
    }

    /// The agent rooted at `entity` left the population at the current tick.
    pub fn record_death(&mut self, entity: Entity) {
        match self.live_ids.remove(&entity) {
            Some(id) => {
                let result = self.tracker.record_destruction(&id, self.tick);
                self.note(result);
                self.departed.push((entity, id));
            }
            None => {
                eprintln!("live lineage: death of an agent with no live id");
                self.faults += 1;
            }
        }
    }

    /// Prune extinct branches and splice unary paths, keeping every open individual.
    fn compact(&mut self) {
        self.compactions += 1;
        match self.tracker.compact(&[]) {
            Ok(report) => {
                self.node_count = report.nodes_after as u64;
                self.next_compaction_at = self.compaction_floor.max(2 * self.node_count);
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

    fn e(n: u32) -> Entity {
        Entity::from_raw(n)
    }

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
        live.record_founder(e(1), "f".into(), MorphologyGenotype::new());
        live.set_tick(5);
        live.record_birth(
            e(2),
            e(1),
            1,
            MorphologyGenotype::new(),
            RelationType::Clone,
        );
        let child = live.live_id(e(2)).unwrap().to_owned();
        assert_eq!(child, "birth-0000000000000001-00000000");
        live.set_tick(9);
        live.record_death(e(2));
        assert_eq!(live.faults(), 0);
        assert_eq!(live.live_id(e(2)), None, "a dead agent leaves the table");
        let (nodes, rels) = live.graph();
        let c = nodes.iter().find(|n| n.id == child).unwrap();
        assert_eq!((c.origin_tick, c.destruction_tick), (Some(5), Some(9)));
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].source_id, "f");
        live.record_death(e(2));
        assert_eq!(live.faults(), 1);
    }

    #[test]
    fn a_parent_that_died_earlier_in_the_same_flush_is_still_named() {
        let mut live = LiveLineage::new(1);
        live.record_founder(e(1), "f".into(), MorphologyGenotype::new());
        live.set_tick(3);
        live.record_death(e(1));
        live.record_birth(
            e(2),
            e(1),
            1,
            MorphologyGenotype::new(),
            RelationType::Clone,
        );
        assert_eq!(live.faults(), 0);
        let (_, rels) = live.graph();
        assert_eq!(rels[0].source_id, "f");
        // Once the tick is over, the dead parent is forgotten.
        live.set_tick(4);
        live.record_birth(
            e(3),
            e(1),
            1,
            MorphologyGenotype::new(),
            RelationType::Clone,
        );
        assert_eq!(live.faults(), 1);
    }

    #[test]
    fn restore_without_a_cursor_continues_after_the_greatest_id_in_the_save() {
        let mut live = LiveLineage::new(7);
        live.record_founder(e(0), "f".into(), MorphologyGenotype::new());
        for n in 1..=3 {
            live.record_birth(
                e(n),
                e(0),
                1,
                MorphologyGenotype::new(),
                RelationType::Clone,
            );
        }
        let (nodes, rels) = live.graph();
        let mut back = LiveLineage::restore(7, None, nodes, rels, &[], &[], 0).unwrap();
        assert_eq!(back.mint_birth_id(), "birth-0000000000000007-00000003");

        let behind = SavedLiveLineage {
            run_id: 7,
            ids_issued: 1,
            next_compaction_at: FIRST_COMPACTION_AT,
            compaction_floor: FIRST_COMPACTION_AT,
            faults: 0,
        };
        let (nodes, rels) = live.graph();
        assert!(LiveLineage::restore(7, Some(&behind), nodes, rels, &[], &[], 0).is_err());
    }

    #[test]
    fn compaction_keeps_open_individuals_without_being_told() {
        let mut live = LiveLineage::new(3);
        live.record_founder(e(0), "f".into(), MorphologyGenotype::new());
        live.record_birth(
            e(1),
            e(0),
            1,
            MorphologyGenotype::new(),
            RelationType::Clone,
        );
        live.record_birth(
            e(2),
            e(0),
            1,
            MorphologyGenotype::new(),
            RelationType::Clone,
        );
        let a = live.live_id(e(1)).unwrap().to_owned();
        let b = live.live_id(e(2)).unwrap().to_owned();
        live.record_death(e(2));
        live.compact();
        let (nodes, _) = live.graph();
        let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&a.as_str()), "the living child is kept");
        assert!(ids.contains(&"f"), "its parent is kept");
        assert!(!ids.contains(&b.as_str()), "the dead leaf is pruned");
        assert_eq!(live.node_count(), 2);
    }

    #[test]
    fn the_compaction_floor_is_injectable_and_compaction_waits_for_the_tick_boundary() {
        let mut live = LiveLineage::new(3);
        live.set_compaction_floor(3);
        live.record_founder(e(0), "f".into(), MorphologyGenotype::new());
        live.record_birth(
            e(1),
            e(0),
            1,
            MorphologyGenotype::new(),
            RelationType::Clone,
        );
        live.record_birth(
            e(2),
            e(0),
            1,
            MorphologyGenotype::new(),
            RelationType::Clone,
        );
        live.record_death(e(2));
        assert_eq!(live.compactions(), 0, "no compaction inside a flush");
        live.set_tick(1);
        assert_eq!(live.compactions(), 1);
        assert_eq!(live.node_count(), 2);
        assert_eq!(live.next_compaction_at(), 4);
        assert_eq!(live.saved().compaction_floor, 3);
    }
}
