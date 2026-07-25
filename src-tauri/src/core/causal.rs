//! Re-export of [`anima_domain::causal`].
//!
//! The causal ledger moved into the `anima-domain` crate (G2 task 1): it is a world law, not an
//! engine detail, and the headless runner and the live world are both meant to be adapters over the
//! same one rather than each carrying a copy.
//!
//! This shim keeps every existing `crate::core::causal::…` path working, so the extraction is a
//! structural change and not a breaking one.
pub use anima_domain::causal::*;
