//! The shared domain core: world laws, the interventions that perturb them, and the causal ledger
//! that records who caused what.
//!
//! Extracted from `anima-engine` as the first step of G2 task 1. The target shape is one domain
//! crate with two adapters over it — the headless experiment runner and the live Bevy world — so
//! that a law expressed once observably alters both, instead of being implemented twice and
//! drifting.
//!
//! What lives here is decided by a single test: **does it depend on an engine?** These modules
//! reference nothing but `serde` and each other. `bevy_ecs`, `tauri` and `burn` are all absent from
//! this crate's manifest on purpose, so the boundary is enforced by the build rather than by
//! convention.
//!
//! `anima-engine` re-exports these under their original `core::` paths, so every existing call site
//! keeps working and the extraction is not a breaking change for the rest of the tree.

pub mod causal;
pub mod intervention;
pub mod laws;
pub mod sim_clock;
pub mod units;
