//! Fitness rules — feature ecological-selection, branch F.
//!
//! Before this module the whole crate had exactly one fitness definition, written inline in
//! `check_epoch_completion_system`: `cumulative_distance + tick_count`. Because
//! `FeatureTracker::tick_count` increments unconditionally every tick, that sum is, after one
//! epoch, essentially a ranking by distance alone — and distance has nothing ecological in it.
//! Branch M gave the world death; this module lets a death-bearing world decide **who passes
//! genes on**.
//!
//! Rules of the house (design §0):
//!
//! - [`LocomotionFitness`] copies the historical line **verbatim**, operand order included. The
//!   two `f32` terms do not commute at rounding level; "simplifying" it can flip a distant
//!   checksum. It exists so `ANIMA_FITNESS` off (and `"locomotion"`) is today's behaviour bit
//!   for bit — that is claim N3.
//! - [`EcologicalFitness`] reads [`AgentEvaluation`] — the counters already computed every tick —
//!   and contains **no distance term**. A distance term smuggles P-04 back in through the back
//!   door: the MAP-Elites second axis would again be fitness minus a constant.

use bevy_ecs::prelude::*;

use crate::ai::hrrl::HomeostaticState;
use crate::core::agent_systems::AgentEvaluation;
use crate::core::components::FeatureTracker;
use crate::evolution::genotype::MorphologyGenotype;

/// The rule the crate has always had, named for provenance.
pub const FITNESS_LOCOMOTION: &str = "locomotion";
/// The ecological rule: survival without being at the bottom, plus net foraging surplus.
pub const FITNESS_ECOLOGICAL: &str = "ecological";

/// Enum copy of the rule id so the setting stays `Copy` and serializable while the trait below
/// carries behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FitnessRuleId {
    Locomotion,
    Ecological,
}

impl FitnessRuleId {
    pub fn id(&self) -> &'static str {
        match self {
            FitnessRuleId::Locomotion => FITNESS_LOCOMOTION,
            FitnessRuleId::Ecological => FITNESS_ECOLOGICAL,
        }
    }
}

/// Which fitness rule ranks agents at epoch replacement. A resource rather than scattered env
/// reads so tests can set it directly and a run's configuration stays visible in one place.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct FitnessRuleSetting {
    pub rule: FitnessRuleId,
    /// Weight of the net-energy surplus term. A world parameter (design §2.2), not a constant.
    pub w_surplus: f32,
}

impl Default for FitnessRuleSetting {
    fn default() -> Self {
        Self {
            rule: FitnessRuleId::Locomotion,
            w_surplus: 1.0,
        }
    }
}

impl FitnessRuleSetting {
    /// Flag-off: today's behaviour, bit for bit.
    pub fn locomotion() -> Self {
        Self::default()
    }

    /// Read `ANIMA_FITNESS` (`locomotion` | `ecological`) and optional `ANIMA_W_SURPLUS`.
    /// Unset or unparseable keeps the legacy behaviour — the safe direction for a flag that
    /// changes what selection *is*.
    pub fn from_env() -> Self {
        let rule = match std::env::var("ANIMA_FITNESS")
            .map(|v| v.trim().to_ascii_lowercase())
            .as_deref()
        {
            Ok(FITNESS_ECOLOGICAL) => FitnessRuleId::Ecological,
            _ => FitnessRuleId::Locomotion,
        };
        let w_surplus = std::env::var("ANIMA_W_SURPLUS")
            .ok()
            .and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|w| w.is_finite() && *w >= 0.0)
            .unwrap_or(1.0);
        Self { rule, w_surplus }
    }

    /// Parse a declared rule id (manifest layer). Unknown ids are refused by the caller's
    /// preflight, never silently mapped to a default.
    pub fn parse_rule(id: &str) -> Option<FitnessRuleId> {
        match id.trim().to_ascii_lowercase().as_str() {
            FITNESS_LOCOMOTION => Some(FitnessRuleId::Locomotion),
            FITNESS_ECOLOGICAL => Some(FitnessRuleId::Ecological),
            _ => None,
        }
    }
}

/// One fitness rule. Two implementations exist and no third should be added casually: every rule
/// is a claim about what this simulator *selects for*, and each must carry its own provenance id.
pub trait FitnessRule {
    fn score(
        &self,
        eval: &AgentEvaluation,
        tracker: &FeatureTracker,
        homeo: &HomeostaticState,
        genotype: &MorphologyGenotype,
    ) -> f32;

    /// Goes into the manifest, the tick-capture export and the ensemble provenance record.
    fn id(&self) -> &'static str;
}

/// The historical line, copied verbatim from `check_epoch_completion_system`
/// (`tracker.cumulative_distance + tracker.tick_count as f32`). Operand order is load-bearing:
/// `f32` addition does not commute at rounding level, and FI-S01 pins the bits.
pub struct LocomotionFitness;

impl FitnessRule for LocomotionFitness {
    fn score(
        &self,
        _eval: &AgentEvaluation,
        tracker: &FeatureTracker,
        _homeo: &HomeostaticState,
        _genotype: &MorphologyGenotype,
    ) -> f32 {
        tracker.cumulative_distance + tracker.tick_count as f32
    }

    fn id(&self) -> &'static str {
        FITNESS_LOCOMOTION
    }
}

/// Survival measured by the one counter with the living condition attached — `survival_ticks`
/// only advances while `energy > 0 && hydration > 0` — plus the net foraging surplus, rewarding
/// animals that acquire more EU than they burn. No distance term, deliberately. Intake is
/// bookkeeping recorded where the ledger itself measures a credit; it moves no energy.
pub struct EcologicalFitness {
    pub w_surplus: f32,
}

impl FitnessRule for EcologicalFitness {
    fn score(
        &self,
        eval: &AgentEvaluation,
        _tracker: &FeatureTracker,
        _homeo: &HomeostaticState,
        _genotype: &MorphologyGenotype,
    ) -> f32 {
        eval.survival_ticks as f32
            + self.w_surplus * (eval.total_energy_gained - eval.total_energy_expended).max(0.0)
    }

    fn id(&self) -> &'static str {
        FITNESS_ECOLOGICAL
    }
}

/// Instantiate the selected rule. Dispatch only; all scoring goes through the trait so a rule's
/// arithmetic lives in exactly one place.
pub fn selected_rule(setting: &FitnessRuleSetting) -> Box<dyn FitnessRule> {
    match setting.rule {
        FitnessRuleId::Locomotion => Box::new(LocomotionFitness),
        FitnessRuleId::Ecological => Box::new(EcologicalFitness {
            w_surplus: setting.w_surplus,
        }),
    }
}
