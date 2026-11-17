//! Re-export of [`anima_domain::energy`].
//!
//! The closed-energy ledger moved into the domain crate (G2 task 1). It is the law G1.1
//! established — `plants + animals + detritus` is invariant absent an explicit source or sink — so
//! both engines must share one implementation rather than each carrying its own.
//!
//! `EcosystemBiomass` is still a Bevy `Resource` here: the domain crate derives it under its `bevy`
//! feature, which this crate enables. That seam is what let a `Resource` move at all — the orphan
//! rule forbids implementing a foreign trait for a foreign type, so without it every ECS type was
//! stuck on the engine side by construction.
pub use anima_domain::energy::*;

/// Settle, through the ledger, EU that has **already left** an `f32` reserve and is bound for
/// detritus: the reproductive overhead, or what a declared epidemic strips from living animals.
/// Returns the EU the ledger moved, which is `amount` for any finite positive `amount`.
///
/// The caller has debited the reserve by exactly `amount` (see [`debit_reserve`]), so the EU exists
/// and the transfer must be funded. [`EnergyLedger::transfer`] clamps to what the *source
/// compartment* holds, and for `Animals` that is `EcosystemBiomass::animals`, which is not a store:
/// it is the last census's snapshot of the reserves plus the credits made since. Before the first
/// census it is zero, so on the first tick of a world every such flow met an empty source, and a
/// routing that only works when the mirror is fresh hands exactly those flows back to a raw
/// `pool.detritus +=`, which conserves EU but leaves the ledger blind to it (G5: 14 of the first 19
/// births of a 20-founder run).
///
/// So when the mirror lags behind the amount it is lifted to cover it before the transfer. That is
/// a correction of a derived view, not of any store: the census overwrites `animals` from the agents'
/// reserves later in the same tick, and the closed total is read from the stores. When the mirror
/// already holds `amount` nothing is lifted and the call is exactly [`EnergyLedger::transfer`]. The
/// first tick is not the only time the mirror can hold less: grazing credits a reserve without
/// crediting the mirror, so it can lag inside any tick, and a lift is possible then too. It is
/// harmless for the same reason.
///
/// Allocation-free.
pub fn settle_reserve_outflow_to_detritus(
    ledger: &mut EnergyLedger,
    pool: &mut EcosystemBiomass,
    amount: f64,
    event: EnergyEvent,
) -> f64 {
    if !amount.is_finite() || amount <= 0.0 {
        return 0.0;
    }
    if pool.animals < amount {
        pool.animals = amount;
    }
    ledger.transfer(
        pool,
        Compartment::Animals,
        Compartment::Detritus,
        amount,
        event,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(animals: f64, detritus: f64) -> EcosystemBiomass {
        EcosystemBiomass {
            detritus,
            plants: 10.0,
            animals,
            dormant: 0.0,
        }
    }

    #[test]
    fn an_outflow_is_settled_even_when_the_animals_mirror_is_stale() {
        let mut ledger = EnergyLedger::default();
        // The first tick: genesis filled the reserves, the census has not snapshotted them yet.
        let mut pool = pool(0.0, 5.0);
        let moved = settle_reserve_outflow_to_detritus(
            &mut ledger,
            &mut pool,
            12.5,
            EnergyEvent::BiologicalReproduction,
        );
        assert_eq!(moved, 12.5);
        assert_eq!(
            pool.detritus, 17.5,
            "detritus must receive the whole amount"
        );
        assert_eq!(ledger.settled_count(), 1);
        assert_eq!(ledger.granted(), 12.5);
        assert_eq!(ledger.refused(), 0.0, "a lagging mirror is not a shortage");
        assert_eq!(pool.animals, 0.0, "the lift is spent by the transfer");
    }

    #[test]
    fn a_fresh_mirror_is_debited_and_never_lifted() {
        let mut ledger = EnergyLedger::default();
        let mut pool = pool(100.0, 5.0);
        let moved = settle_reserve_outflow_to_detritus(
            &mut ledger,
            &mut pool,
            12.5,
            EnergyEvent::Intervention,
        );
        assert_eq!(moved, 12.5);
        assert_eq!(
            pool.animals, 87.5,
            "the mirror follows the flow, as transfer does"
        );
        assert_eq!(pool.detritus, 17.5);
        assert_eq!(ledger.settled_count(), 1);
    }

    #[test]
    fn a_non_positive_or_non_finite_outflow_moves_nothing() {
        for amount in [0.0, -3.0, f64::NAN, f64::INFINITY] {
            let mut ledger = EnergyLedger::default();
            let mut pool = pool(40.0, 5.0);
            let moved = settle_reserve_outflow_to_detritus(
                &mut ledger,
                &mut pool,
                amount,
                EnergyEvent::Intervention,
            );
            assert_eq!(moved, 0.0, "amount {amount}");
            assert_eq!(pool, self::pool(40.0, 5.0), "amount {amount}");
            assert_eq!(ledger.settled_count(), 0, "amount {amount}");
        }
    }
}
