//! The exotic-energy budget ledger (AE-204). MU is audited here, separately from the closed EU
//! ledger.

use super::*;

// ---- Budget ledger (AE-204) ------------------------------------------------------------------

/// An audit of where every MU is. The closure equation (design §"Dynamic exotic field and budget"):
///
/// ```text
/// initial + sourced  =  field + organism_storage + dissipated + exported  ± tolerance
/// ```
///
/// [`balance_error`](Self::balance_error) is the left minus the right; a well-behaved subsystem keeps
/// it within the documented numeric tolerance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExoticEnergyBudget {
    pub unit: UnitId,
    /// MU present at genesis.
    pub initial: f64,
    /// MU injected by the source over the run.
    pub sourced: f64,
    /// MU currently held in the spatial field.
    pub field: f64,
    /// MU currently held in organism storage (a reference-model scalar in this slice).
    pub organism_storage: f64,
    /// MU removed by decay / spend (a declared sink).
    pub dissipated: f64,
    /// MU that left the modelled system (e.g. off-grid at an open boundary).
    pub exported: f64,
}

impl ExoticEnergyBudget {
    /// `(initial + sourced) − (field + storage + dissipated + exported)`. Zero means perfect MU
    /// conservation.
    pub fn balance_error(&self) -> f64 {
        (self.initial + self.sourced)
            - (self.field + self.organism_storage + self.dissipated + self.exported)
    }

    /// A scale for a *relative* balance check: the total MU that has ever entered the system.
    pub fn throughput(&self) -> f64 {
        (self.initial + self.sourced).max(1.0)
    }
}
