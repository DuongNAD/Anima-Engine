//! The single place energy units (EU) move in the live world.
//!
//! `SIMULATION_RULES.md` and [`crate::core::sim_rules::STATE_VARIABLES`] declare `plants`,
//! `animals` and `detritus` as [`Conservation::ClosedEnergy`](crate::core::sim_rules::Conservation)
//! members: absent an explicit source or sink their sum is invariant. Until this module existed the
//! live Bevy loop did not honour that. Food entities and tree fruit materialised EU from nothing and
//! handed it straight to an animal, and epoch replacement returned a corpse's reserve to detritus
//! and *then* granted the replacement a fresh flat reserve — so a long run drifted upward with no
//! record of by how much or from where.
//!
//! # Where the energy actually lives
//!
//! Two of the three compartments are mirrors, not storage:
//!
//! | Compartment | Authoritative store | [`EcosystemBiomass`] field |
//! |---|---|---|
//! | plants | [`ResourceField::r`](crate::core::ecology::ResourceField) cells | mirror, refreshed per tick |
//! | animals | each agent's `HomeostaticState::energy` | mirror, refreshed per tick |
//! | detritus | `EcosystemBiomass::detritus` | **authoritative** |
//!
//! A transfer between two authoritative stores (grazing the field into an animal, metabolism
//! burning an animal into detritus, predation splitting prey between predator and detritus) is
//! conserved by construction and does not need this module. What needs this module is any move
//! whose *source* would otherwise be nothing at all. Those are the only calls to
//! [`EnergyLedger::transfer_into_reserve`].
//!
//! # Why the destination is measured rather than assumed
//!
//! Agent reserves and field cells are `f32`. Adding a small amount to a large reserve rounds, so
//! "debit the source by `x`, credit the destination by `x`" quietly destroys (or creates) a
//! fraction of an ULP on every single feeding event. Over millions of ticks that is not noise, it
//! is a trend.
//!
//! So the transfer applies to the destination **first**, measures how much the destination actually
//! changed in `f64`, and debits the source by exactly that. Rounding therefore cannot leak: an
//! amount that failed to land in the reserve is simply never withdrawn. This is what lets the
//! conservation gate assert a residual at the tolerance documented in
//! [`RESIDUAL_ABS_TOLERANCE_EU`] instead of an empirical fudge factor.
//!
//! # Genesis is a boundary condition, not a transaction
//!
//! Invariant **D06** of `docs/reference/CREATURE_DEVELOPMENT_CONTRACT.md`: at `t = 0` the
//! population's energy *is* the boundary condition, and the closed-EU baseline is locked **after**
//! plants, animals and detritus are initialised. Founders are therefore not funded out of detritus
//! — they are part of what the baseline measures. Only post-genesis creation is a leak.
//!
//! Epoch replacement is explicitly *not* a birth (D06 again): the replacement is funded from the
//! individual it replaces, whose reserve has just been returned to detritus. When detritus cannot
//! cover the full reserve the replacement starts hungry, and the shortfall is counted in
//! [`EnergyLedger::refused`] rather than conjured.
//!
//! # Hot path
//!
//! Everything here is scalar `f64`/`f32` arithmetic on a resource that owns no heap. The tick
//! systems that call it keep their `allocs == 0` assertions.

use crate::core::ecology::EcosystemBiomass;
use bevy_ecs::prelude::Resource;

/// A compartment of the closed energy system.
///
/// These are exactly the three [`Conservation::ClosedEnergy`](crate::core::sim_rules::Conservation)
/// state variables. There is deliberately no fourth: uneaten food entities and unpicked fruit are
/// *claims on detritus* that settle when something eats them, not a store of their own. Adding a
/// compartment here would mean adding an undeclared quantity to the units table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compartment {
    /// Standing plant biomass.
    Plants,
    /// Energy embodied in living animals.
    Animals,
    /// Free energy available to regrow plants, and the pool food and fruit draw on.
    Detritus,
}

/// Why energy moved. Carried on every transaction so a leak can be attributed to a system rather
/// than merely detected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnergyEvent {
    /// An animal ate a spawned food item.
    Feeding,
    /// An animal ate fruit off a tree.
    Fruiting,
    /// Epoch replacement funded a replacement individual (D06: not a birth).
    Replacement,
    /// A declared intervention moved energy.
    Intervention,
}

/// Absolute tolerance, in EU, for the closed-energy residual of a whole live run.
///
/// **This is a property of the arithmetic, not a knob.** Because every ledger transfer debits the
/// source by the destination's *measured* `f64` delta, an individual transfer is exact — it cannot
/// contribute drift no matter how badly the `f32` add rounds. The residual a real run accumulates
/// therefore comes only from the transfers this module does not mediate, which are the ones between
/// two authoritative stores, and those are summed in `f64` from exact `f32` differences.
///
/// The value below is the slack for the remaining `f32`→`f64` widening in the census sum, at the
/// scale the MVP world runs: ~10^4 EU total across ~10^2 stores, `f64` epsilon 2.2e-16, over
/// ~10^6 accumulation steps. That bounds the honest error near 1e-6 EU; 1e-3 EU is three orders of
/// margin above it and still ~10^-7 of a typical world total, so a real leak of even one food item
/// (tens of EU) is caught by many orders of magnitude.
///
/// Do not raise this to make a test pass. A residual above it means energy is being created or
/// destroyed somewhere outside this module, and the fix is to route that site through here.
pub const RESIDUAL_ABS_TOLERANCE_EU: f64 = 1e-3;

/// The one API through which EU may be created into a compartment from another compartment.
///
/// Insert it as a resource next to [`EcosystemBiomass`]. Systems that need to fund something take
/// `ResMut<EnergyLedger>` alongside `ResMut<EcosystemBiomass>`.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct EnergyLedger {
    /// Total closed EU measured once, immediately after genesis. `None` until locked.
    baseline: Option<f64>,
    /// Cumulative EU actually moved through this ledger.
    granted: f64,
    /// Cumulative EU that was demanded but could not be funded, because the source compartment
    /// did not hold it. This is the number that says "the world is running an energy deficit"; it
    /// is not an error, it is ecology.
    refused: f64,
    /// How many transactions have settled.
    settled: u64,
    /// Closed EU as of the most recent census. Lets the invariant be read off a running world
    /// rather than only computed at the end of a test.
    last_total: f64,
}

impl EnergyLedger {
    /// Lock the closed-EU baseline. Call exactly once, after genesis has initialised plants,
    /// animals and detritus (D06). Calling it again is ignored, so a restore path cannot silently
    /// re-baseline a run and hide a leak that has already happened.
    pub fn lock_baseline(&mut self, total_eu: f64) {
        if self.baseline.is_none() {
            self.baseline = Some(total_eu);
        }
    }

    /// The baseline, if genesis has locked it.
    pub fn baseline(&self) -> Option<f64> {
        self.baseline
    }

    /// Signed difference between the world's current closed EU and the locked baseline. Positive
    /// means energy was created. Returns `0.0` before the baseline is locked, since there is
    /// nothing to compare against yet.
    pub fn residual(&self, current_total_eu: f64) -> f64 {
        match self.baseline {
            Some(b) => current_total_eu - b,
            None => 0.0,
        }
    }

    /// Whether the run is still conserving energy within [`RESIDUAL_ABS_TOLERANCE_EU`].
    pub fn conserves(&self, current_total_eu: f64) -> bool {
        self.residual(current_total_eu).abs() <= RESIDUAL_ABS_TOLERANCE_EU
    }

    /// Cumulative EU moved through the ledger.
    pub fn granted(&self) -> f64 {
        self.granted
    }

    /// Cumulative EU demanded that the source could not fund.
    pub fn refused(&self) -> f64 {
        self.refused
    }

    /// Number of settled transactions.
    pub fn settled_count(&self) -> u64 {
        self.settled
    }

    /// Record the closed EU the per-tick census just measured, so the invariant is observable on a
    /// live world. Called by `ecosystem_census_system`.
    pub fn observe(&mut self, current_total_eu: f64) {
        self.last_total = current_total_eu;
    }

    /// Closed EU as of the most recent [`observe`](Self::observe).
    pub fn last_total(&self) -> f64 {
        self.last_total
    }

    /// Residual as of the most recent [`observe`](Self::observe).
    pub fn last_residual(&self) -> f64 {
        self.residual(self.last_total)
    }

    /// Move up to `desired` EU out of `from` and into an `f32` reserve that saturates at `cap`.
    ///
    /// Returns the EU actually moved, which is the reserve's measured `f64` growth. The source is
    /// debited by exactly that, so `f32` rounding in the reserve can neither create nor destroy
    /// energy — see the module docs. Anything demanded but unavailable is added to
    /// [`refused`](Self::refused).
    ///
    /// Allocation-free.
    pub fn transfer_into_reserve(
        &mut self,
        pool: &mut EcosystemBiomass,
        from: Compartment,
        reserve: &mut f32,
        cap: f32,
        desired: f32,
        event: EnergyEvent,
    ) -> f64 {
        if !desired.is_finite() || desired <= 0.0 {
            return 0.0;
        }
        let available = Self::compartment(pool, from).max(0.0);
        if available <= 0.0 {
            self.refused += desired as f64;
            return 0.0;
        }
        // Never offer the reserve more than the source can actually fund.
        let offered = (desired as f64).min(available) as f32;

        let before = *reserve as f64;
        *reserve = (*reserve + offered).min(cap);
        let applied = (*reserve as f64) - before;

        if applied <= 0.0 {
            // The reserve was already at its cap, or the add rounded to nothing. Nothing is
            // withdrawn, so nothing is lost.
            return 0.0;
        }

        *Self::compartment_mut(pool, from) -= applied;
        Self::credit_mirror(pool, Compartment::Animals, applied);

        self.granted += applied;
        self.settled += 1;
        let shortfall = desired as f64 - applied;
        if shortfall > 0.0 {
            self.refused += shortfall;
        }
        let _ = event;
        applied
    }

    /// Move `amount` EU between two compartments of the ledger's own bookkeeping, clamped to what
    /// `from` holds. For moves where neither side is an `f32` reserve — a declared intervention,
    /// for instance. Returns the amount actually moved.
    ///
    /// Allocation-free.
    pub fn transfer(
        &mut self,
        pool: &mut EcosystemBiomass,
        from: Compartment,
        to: Compartment,
        amount: f64,
        event: EnergyEvent,
    ) -> f64 {
        if !amount.is_finite() || amount <= 0.0 || from == to {
            return 0.0;
        }
        let available = Self::compartment(pool, from).max(0.0);
        let moved = amount.min(available);
        if moved <= 0.0 {
            self.refused += amount;
            return 0.0;
        }
        *Self::compartment_mut(pool, from) -= moved;
        *Self::compartment_mut(pool, to) += moved;
        self.granted += moved;
        self.settled += 1;
        if amount - moved > 0.0 {
            self.refused += amount - moved;
        }
        let _ = event;
        moved
    }

    /// Keep the `animals` mirror in step between census runs. `plants` is rebuilt from the resource
    /// field every tick and `detritus` is authoritative, so only `animals` needs nudging here.
    #[inline]
    fn credit_mirror(pool: &mut EcosystemBiomass, to: Compartment, amount: f64) {
        if to == Compartment::Animals {
            pool.animals += amount;
        }
    }

    #[inline]
    fn compartment(pool: &EcosystemBiomass, c: Compartment) -> f64 {
        match c {
            Compartment::Plants => pool.plants,
            Compartment::Animals => pool.animals,
            Compartment::Detritus => pool.detritus,
        }
    }

    #[inline]
    fn compartment_mut(pool: &mut EcosystemBiomass, c: Compartment) -> &mut f64 {
        match c {
            Compartment::Plants => &mut pool.plants,
            Compartment::Animals => &mut pool.animals,
            Compartment::Detritus => &mut pool.detritus,
        }
    }
}

/// Add `amount` to an `f32` reserve, saturating at `cap`, and return the **exact** `f64` amount the
/// reserve grew by.
///
/// Use this on the destination of any store-to-store transfer. `reserve + amount` rounds, and the
/// saturation can clip, so the number that actually landed is not the number that was asked for.
/// Debiting the source by the asked-for amount is what makes a "conserved by construction"
/// transfer leak a fraction of an ULP every time it runs.
#[inline]
pub fn credit_reserve(reserve: &mut f32, amount: f32, cap: f32) -> f64 {
    if !amount.is_finite() || amount <= 0.0 {
        return 0.0;
    }
    let before = *reserve as f64;
    *reserve = (*reserve + amount).min(cap);
    (*reserve as f64) - before
}

/// Subtract `amount` from an `f32` reserve, flooring at zero, and return the **exact** `f64` amount
/// the reserve shrank by. The counterpart of [`credit_reserve`] for the source of a transfer.
#[inline]
pub fn debit_reserve(reserve: &mut f32, amount: f32) -> f64 {
    if !amount.is_finite() || amount <= 0.0 {
        return 0.0;
    }
    let before = *reserve as f64;
    *reserve = (*reserve - amount).max(0.0);
    before - (*reserve as f64)
}

/// The world's total closed EU, summed from the three **authoritative** stores rather than from the
/// [`EcosystemBiomass`] mirrors. This is the number the conservation gate compares to the baseline.
///
/// `plant_biomass` is `ResourceField::total_biomass()`, `animal_energy` is the sum of every living
/// agent's reserve, and `detritus` is `EcosystemBiomass::detritus`.
#[inline]
pub fn closed_total_eu(plant_biomass: f64, animal_energy: f64, detritus: f64) -> f64 {
    plant_biomass + animal_energy + detritus
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(detritus: f64, plants: f64, animals: f64) -> EcosystemBiomass {
        EcosystemBiomass {
            detritus,
            plants,
            animals,
        }
    }

    #[test]
    fn transfer_into_reserve_debits_exactly_what_the_reserve_gained() {
        let mut ledger = EnergyLedger::default();
        let mut p = pool(100.0, 0.0, 0.0);
        let mut reserve = 0.0f32;

        let moved = ledger.transfer_into_reserve(
            &mut p,
            Compartment::Detritus,
            &mut reserve,
            1000.0,
            25.0,
            EnergyEvent::Feeding,
        );

        assert_eq!(moved, 25.0);
        assert_eq!(reserve, 25.0);
        assert_eq!(p.detritus, 75.0);
        // Total across the mirrors is unchanged: detritus lost exactly what animals gained.
        assert_eq!(p.total(), 100.0);
    }

    #[test]
    fn a_reserve_at_its_cap_withdraws_nothing() {
        let mut ledger = EnergyLedger::default();
        let mut p = pool(100.0, 0.0, 0.0);
        let mut reserve = 50.0f32;

        let moved = ledger.transfer_into_reserve(
            &mut p,
            Compartment::Detritus,
            &mut reserve,
            50.0, // already at cap
            10.0,
            EnergyEvent::Feeding,
        );

        assert_eq!(moved, 0.0);
        assert_eq!(reserve, 50.0);
        assert_eq!(
            p.detritus, 100.0,
            "a refused grant must not touch the source"
        );
    }

    #[test]
    fn rounding_in_a_large_reserve_never_creates_energy() {
        // 1e7 is large enough that adding 1e-4 rounds away entirely in f32. The naive
        // "debit source by desired, credit destination by desired" would destroy that amount on
        // every call; measuring the destination means we simply never withdraw it.
        let mut ledger = EnergyLedger::default();
        let mut p = pool(1000.0, 0.0, 1.0e7);
        let mut reserve = 1.0e7f32;
        let detritus_before = p.detritus;

        for _ in 0..10_000 {
            ledger.transfer_into_reserve(
                &mut p,
                Compartment::Detritus,
                &mut reserve,
                f32::MAX,
                1.0e-4,
                EnergyEvent::Feeding,
            );
        }

        assert_eq!(
            reserve, 1.0e7f32,
            "the add rounds away, as expected at this scale"
        );
        assert_eq!(
            p.detritus, detritus_before,
            "nothing landed, so nothing may be withdrawn"
        );
    }

    #[test]
    fn a_grant_larger_than_the_source_is_capped_and_the_shortfall_recorded() {
        let mut ledger = EnergyLedger::default();
        let mut p = pool(10.0, 0.0, 0.0);
        let mut reserve = 0.0f32;

        let moved = ledger.transfer_into_reserve(
            &mut p,
            Compartment::Detritus,
            &mut reserve,
            1000.0,
            100.0,
            EnergyEvent::Replacement,
        );

        assert_eq!(moved, 10.0);
        assert_eq!(reserve, 10.0);
        assert_eq!(p.detritus, 0.0);
        assert!(
            (ledger.refused() - 90.0).abs() < 1e-9,
            "the 90 EU that could not be funded is recorded, not conjured: {}",
            ledger.refused()
        );
    }

    #[test]
    fn an_empty_source_funds_nothing() {
        let mut ledger = EnergyLedger::default();
        let mut p = pool(0.0, 0.0, 0.0);
        let mut reserve = 5.0f32;

        let moved = ledger.transfer_into_reserve(
            &mut p,
            Compartment::Detritus,
            &mut reserve,
            100.0,
            50.0,
            EnergyEvent::Fruiting,
        );

        assert_eq!(moved, 0.0);
        assert_eq!(reserve, 5.0);
        assert_eq!(ledger.refused(), 50.0);
    }

    #[test]
    fn baseline_locks_once_so_a_reload_cannot_hide_an_existing_leak() {
        let mut ledger = EnergyLedger::default();
        assert_eq!(ledger.baseline(), None);
        assert_eq!(
            ledger.residual(12345.0),
            0.0,
            "no baseline, nothing to compare"
        );

        ledger.lock_baseline(1000.0);
        assert_eq!(ledger.baseline(), Some(1000.0));

        ledger.lock_baseline(2000.0);
        assert_eq!(ledger.baseline(), Some(1000.0), "second lock is ignored");
        assert_eq!(ledger.residual(1000.5), 0.5);
        assert!(ledger.conserves(1000.0 + RESIDUAL_ABS_TOLERANCE_EU));
        assert!(!ledger.conserves(1000.0 + 10.0 * RESIDUAL_ABS_TOLERANCE_EU));
    }

    #[test]
    fn plain_transfer_moves_between_compartments_and_conserves_the_total() {
        let mut ledger = EnergyLedger::default();
        let mut p = pool(100.0, 500.0, 50.0);
        let before = p.total();

        let moved = ledger.transfer(
            &mut p,
            Compartment::Detritus,
            Compartment::Plants,
            30.0,
            EnergyEvent::Intervention,
        );

        assert_eq!(moved, 30.0);
        assert_eq!(p.detritus, 70.0);
        assert_eq!(p.plants, 530.0);
        assert_eq!(p.total(), before);
    }

    #[test]
    fn transfer_rejects_nonsense_amounts_and_self_moves() {
        let mut ledger = EnergyLedger::default();
        let mut p = pool(100.0, 0.0, 0.0);

        for amount in [0.0, -5.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                ledger.transfer(
                    &mut p,
                    Compartment::Detritus,
                    Compartment::Plants,
                    amount,
                    EnergyEvent::Intervention,
                ),
                0.0
            );
        }
        assert_eq!(
            ledger.transfer(
                &mut p,
                Compartment::Detritus,
                Compartment::Detritus,
                10.0,
                EnergyEvent::Intervention,
            ),
            0.0,
            "a compartment cannot fund itself"
        );
        assert_eq!(p.detritus, 100.0);
        assert_eq!(ledger.settled_count(), 0);
    }

    #[test]
    fn closed_total_sums_the_authoritative_stores() {
        assert_eq!(closed_total_eu(500.0, 250.0, 100.0), 850.0);
    }
}
