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
