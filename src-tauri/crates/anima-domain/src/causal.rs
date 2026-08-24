//! # Causal ledger (M2.3) — "what changed, by how much, and *why*".
//!
//! Every meaningful change to the world is recorded as an [`EffectRecord`] that names the cause it
//! came from and the parent effect that triggered it, so any observed change can be traced back
//! along its mechanism chain to a **root cause** (scenario S12). This is the data structure that
//! turns "many subsystems that each changed something" into "one explainable cause→effect world".
//!
//! It is deliberately pure and Bevy-free (no `HashMap`, ordered `Vec` storage) so it is cheap to
//! test and **deterministic**: the same run produces the same ledger, which is what makes scenario
//! replay (S13) and control/treatment deltas (S14) meaningful.

use serde::{Deserialize, Serialize};

/// Identifies the ultimate cause of a chain — an [`crate::intervention::InterventionCommand`]
/// (a user intervention or scenario event). Assigned by the scenario; `0` is reserved for
/// "background / no explicit cause".
pub type CauseId = u32;

/// Reserved cause id for changes with no explicit intervention behind them (baseline dynamics).
pub const CAUSE_BACKGROUND: CauseId = 0;

/// Reserved cause id for a live human observer (ADR-0004 O2).
///
/// At the **top** of the range on purpose. Scenario ids are author-assigned by hand from the bottom
/// (`cause_id: 1`, `2`, …) with no allocator anywhere, so a constant picked from down there could
/// silently come to mean two different things in one ledger — the observer's doing and a scenario
/// forcing, sharing a root. `u32::MAX` is somewhere no hand-written manifest lands by accident, and
/// [`ExperimentManifest::validate`](../../../src/core/experiment.rs) refuses a scenario intervention
/// that claims it anyway rather than trusting that.
pub const CAUSE_OBSERVER: CauseId = CauseId::MAX;

/// Whether `id` belongs to the engine rather than to a scenario author.
///
/// Reserved ids carry meaning the ledger relies on: [`CAUSE_BACKGROUND`] means "no explicit cause"
/// and [`CAUSE_OBSERVER`] means "a human did this". A scenario reusing either would make
/// [`CausalLedger::root_cause`] answer a different question than the one asked.
pub fn is_reserved_cause(id: CauseId) -> bool {
    id == CAUSE_BACKGROUND || id == CAUSE_OBSERVER
}

/// Index of an [`EffectRecord`] in a [`CausalLedger`]. Equal to the record's position, so lookups
/// are O(1) and stable across a run.
pub type EffectId = u32;

/// One recorded change: a quantified delta to some `target`, attributable to `cause_id` and
/// (optionally) an immediate upstream `parent` effect.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EffectRecord {
    pub effect_id: EffectId,
    /// The root cause this effect ultimately serves (propagated down a chain).
    pub cause_id: CauseId,
    /// The immediate upstream effect, or `None` if this effect is applied directly by the cause.
    pub parent: Option<EffectId>,
    pub tick: u64,
    /// What changed, e.g. `"npp"`, `"herbivores"`, `"precip@cell(3,4)"`.
    pub target: String,
    /// The value of `target` after this effect.
    pub quantity: f64,
    /// The change applied by this effect (`quantity` after − before).
    pub delta: f64,
    /// Human-readable mechanism — the "why" shown to the user.
    pub mechanism: String,
}

/// An append-only ledger of effects for a single run. Effect ids are dense and equal to the storage
/// index, so parent links are just indices and traversal is allocation-light.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CausalLedger {
    effects: Vec<EffectRecord>,
}

impl CausalLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an effect and return its new [`EffectId`]. When `parent` is given, the record inherits
    /// the parent's `cause_id` so a whole chain shares one root cause automatically (a caller may
    /// still pass an explicit `cause_id` when there is no parent).
    ///
    /// Eight parameters, deliberately. Each one is a distinct field of the provenance record and a
    /// caller must supply all of them — bundling them into a struct would only move the same eight
    /// values one line up, while making it easier to construct a half-filled record. The lint fires
    /// now only because the module became its own crate and is linted on its own.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        cause_id: CauseId,
        parent: Option<EffectId>,
        tick: u64,
        target: impl Into<String>,
        quantity: f64,
        delta: f64,
        mechanism: impl Into<String>,
    ) -> EffectId {
        // A parent's cause always wins, so the root cause can never be lost partway down a chain.
        let resolved_cause = match parent.and_then(|p| self.effects.get(p as usize)) {
            Some(parent_rec) => parent_rec.cause_id,
            None => cause_id,
        };
        let effect_id = self.effects.len() as EffectId;
        self.effects.push(EffectRecord {
            effect_id,
            cause_id: resolved_cause,
            parent,
            tick,
            target: target.into(),
            quantity,
            delta,
            mechanism: mechanism.into(),
        });
        effect_id
    }

    pub fn get(&self, id: EffectId) -> Option<&EffectRecord> {
        self.effects.get(id as usize)
    }

    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn all(&self) -> &[EffectRecord] {
        &self.effects
    }

    /// Walk the parent links from `id` up to the top of its chain, returning the effect ids from
    /// `id` (first) to the root effect (last). Robust to a cycle (bounded by ledger length) so a
    /// corrupt parent link can never spin forever.
    pub fn trace_to_root(&self, id: EffectId) -> Vec<EffectId> {
        let mut chain = Vec::new();
        let mut cur = Some(id);
        while let Some(c) = cur {
            let Some(rec) = self.get(c) else { break };
            chain.push(c);
            // A valid parent is always a strictly-earlier record (`record()` only ever links to an
            // already-stored effect, so `parent < effect_id`). A non-decreasing parent index can
            // therefore only come from a corrupt/hand-built ledger; stop rather than loop.
            cur = match rec.parent {
                Some(p) if p < c => Some(p),
                _ => None,
            };
        }
        chain
    }

    /// The root cause of `id`: the `cause_id` carried by the top of its chain (equal to the record's
    /// own `cause_id`, since a parent's cause is propagated at record time).
    pub fn root_cause(&self, id: EffectId) -> Option<CauseId> {
        self.get(id).map(|r| r.cause_id)
    }

    /// The top-`k` effects on `target` within the inclusive tick `window`, ranked by absolute delta
    /// (largest first) — the "why did this change?" contributors. Ties keep ledger order (stable).
    pub fn top_contributors(
        &self,
        target: &str,
        window: (u64, u64),
        k: usize,
    ) -> Vec<&EffectRecord> {
        let (lo, hi) = window;
        let mut hits: Vec<&EffectRecord> = self
            .effects
            .iter()
            .filter(|r| r.target == target && r.tick >= lo && r.tick <= hi)
            .collect();
        // Stable sort by descending |delta|; ties preserve insertion order.
        hits.sort_by(|a, b| {
            b.delta
                .abs()
                .partial_cmp(&a.delta.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(k);
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s12_chain_traces_to_root_cause() {
        let mut led = CausalLedger::new();
        let cause: CauseId = 7;
        // intervention → A → B → C, with an unrelated effect interleaved to prove isolation.
        let a = led.record(
            cause,
            None,
            10,
            "precip",
            0.7,
            -0.3,
            "rainfall intervention",
        );
        let _noise = led.record(99, None, 10, "unrelated", 1.0, 0.0, "background");
        let b = led.record(
            CAUSE_BACKGROUND,
            Some(a),
            12,
            "npp",
            0.5,
            -0.2,
            "less rain → less NPP",
        );
        let c = led.record(
            CAUSE_BACKGROUND,
            Some(b),
            18,
            "herbivores",
            40.0,
            -6.0,
            "less forage → fewer herbivores",
        );

        // The chain from C climbs C → B → A and never touches the unrelated effect.
        assert_eq!(led.trace_to_root(c), vec![c, b, a]);
        // The root cause survives the whole chain even though B and C were recorded with BACKGROUND.
        assert_eq!(led.root_cause(c), Some(cause));
        assert_eq!(led.root_cause(b), Some(cause));
        assert_eq!(led.root_cause(a), Some(cause));
    }

    #[test]
    fn s12_top_contributors_ranks_by_absolute_delta_in_window() {
        let mut led = CausalLedger::new();
        led.record(1, None, 5, "npp", 0.9, -0.1, "a");
        led.record(1, None, 8, "npp", 0.5, -0.4, "b"); // biggest
        led.record(1, None, 8, "npp", 0.6, 0.3, "c");
        led.record(1, None, 50, "npp", 0.1, -0.9, "out of window");
        led.record(1, None, 8, "other", 0.0, -0.5, "wrong target");

        let top = led.top_contributors("npp", (0, 20), 2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].mechanism, "b"); // |−0.4| largest in-window npp
        assert_eq!(top[1].mechanism, "c"); // |0.3| next
    }

    #[test]
    fn record_without_parent_keeps_explicit_cause() {
        let mut led = CausalLedger::new();
        let e = led.record(42, None, 0, "x", 1.0, 1.0, "m");
        assert_eq!(led.root_cause(e), Some(42));
        assert_eq!(led.trace_to_root(e), vec![e]);
    }
}
