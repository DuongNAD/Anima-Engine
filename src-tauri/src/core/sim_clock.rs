//! Re-export of [`anima_domain::sim_clock`].
//!
//! The multi-rate clock is a *schedule*, which the G2 target shape puts in the domain crate beside
//! the laws it paces. See [`crate::core::causal`] for why the extraction is shaped this way.
pub use anima_domain::sim_clock::*;
