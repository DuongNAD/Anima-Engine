//! The unit vocabulary — the most fundamental law in the system.
//!
//! `SIMULATION_RULES.md` and `docs/reference/ENERGY_LEDGER_CONTRACT.md` both turn on one
//! distinction: **MU is not EU**. Exotic energy is audited separately from the closed-energy ledger,
//! and an exotic source that claimed the EU unit would silently merge two budgets that the whole
//! experiment design keeps apart (ER04).
//!
//! These types live in the domain crate because the headless runner and the live world must mean
//! the same thing by "EU". A unit vocabulary that each engine defined for itself is exactly the
//! second source of truth G2 exists to remove.
//!
//! The newtypes are the enforcement: unit mixing is a type error rather than a silent string
//! coincidence.

use serde::{Deserialize, Serialize};

/// The canonical biomass-equivalent energy unit. An exotic law may never claim it.
pub const EU_UNIT: &str = "EU";
/// The MVP exotic-energy unit — "mana unit". Distinct from [`EU_UNIT`] by contract (ER04).
pub const MU_UNIT: &str = "MU";

/// A stable identifier for an exotic energy source (e.g. `"arcane_flux"`). Newtype so it can never
/// be confused with a display name or a unit string.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EnergySourceId(pub String);

impl EnergySourceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A stable unit identifier (e.g. `"MU"`, `"EU"`). Newtype so unit mixing is a type error, not a
/// silent string coincidence.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct UnitId(pub String);

impl UnitId {
    pub fn new(u: impl Into<String>) -> Self {
        Self(u.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// Whether this unit is the closed-EU biomass-equivalent unit (which an exotic source may not
    /// use).
    pub fn is_eu(&self) -> bool {
        self.0 == EU_UNIT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mu_is_not_eu() {
        assert_ne!(EU_UNIT, MU_UNIT);
        assert!(UnitId::new(EU_UNIT).is_eu());
        assert!(!UnitId::new(MU_UNIT).is_eu());
    }
}
