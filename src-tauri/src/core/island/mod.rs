//! Shared-island host policy, provenance identifiers, genesis-phase provenance, and
//! zero-allocation guest intent ingress (`ADR-0014` / `QD-SHAREDISLAND` / `QD-GENESIS`, Card I1).
//!
//! Everything here is strictly **opt-in**: runs that never attach [`IslandManifest`] or insert
//! [`SharedGuestIntents`] / [`DrainedGuestIntents`] behave identically to solo trunk execution.

use crate::core::experiment::{Canon, ExperimentManifest};
use crate::core::observer::ObserverPolicy;
use crate::core::world_artifact::WorldIdentity;
use anima_domain::causal::{island_guest_cause_id, CauseId};
use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::fmt;

pub mod inhabit;
pub use inhabit::*;
pub mod intents;
pub use intents::*;
pub mod stream;
pub use stream::*;

/// Trial & Milestone 1 shared-island target animal count (`QD-SCALE-TRIAL`: ~100 individual animals).
pub const DEFAULT_ISLAND_TARGET_ANIMALS: u32 = 100;

/// Decided default LAN concurrent guest cap (`ISLAND-D3(a)`).
pub const DEFAULT_ISLAND_MAX_GUESTS: u16 = 16;

/// Hard socket ceiling inherited from networking limits (`MAX_MIGRATION_WEBSOCKET_CONNECTIONS`).
pub const MAX_ISLAND_GUESTS_CEILING: u16 = 64;

/// Default simulation tick rate assumed by real-time ecology timing calculations (`30 Hz`).
pub const DEFAULT_SIM_TICKS_PER_SECOND: u32 = 30;

/// Decided day/night cycle duration under real-time island play (`D-TIME-1`: 20 minutes).
pub const DEFAULT_DAY_LENGTH_SECONDS: u32 = 20 * 60;

/// Decided seasonal cycle duration under real-time island play (`D-TIME-2`: 90 minutes).
pub const DEFAULT_YEAR_LENGTH_SECONDS: u32 = 90 * 60;

/// Default real-time creature lifespan midpoint (`QD-PLAYRULES` Rule 1: ~1-2 h, 90 min default).
pub const DEFAULT_CREATURE_LIFESPAN_SECONDS: u32 = 90 * 60;

/// Default fraction of lifespan spent reaching maturity (`15%`).
pub const DEFAULT_MATURATION_FRACTION: f32 = 0.15;

/// Default gestation / reproduction cooldown duration in real-time seconds (`10 min`).
pub const DEFAULT_GESTATION_COOLDOWN_SECONDS: u32 = 10 * 60;

const fn default_true() -> bool {
    true
}

/// Three-phase organism genesis declaration (`QD-GENESIS`, ADR-0014).
///
/// Stamped in the island/genesis manifest, [`WorldIdentity::genesis_phase`], and
/// [`ExperimentManifest::fingerprint`]. Experiments must never mix phases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenesisPhase {
    /// `G1`: Fixed, declared founders (`"designed founders"`) for the Milestone 1 demo.
    G1,
    /// `G2`: Viability-screened founder pool, producers-first + multi-pool robustness check (`I11`).
    G2,
    /// `G3`: Separate opt-in emergent primordial organisms research spike (no chemistry OOL).
    G3,
}

impl GenesisPhase {
    /// Stable single-byte domain tag for canonical [`ExperimentManifest::fingerprint`] encoding.
    pub const fn canon_tag(self) -> u8 {
        match self {
            Self::G1 => 0x01,
            Self::G2 => 0x02,
            Self::G3 => 0x03,
        }
    }

    /// Short uppercase phase code (`"G1"`, `"G2"`, `"G3"`).
    pub const fn code(self) -> &'static str {
        match self {
            Self::G1 => "G1",
            Self::G2 => "G2",
            Self::G3 => "G3",
        }
    }

    /// Honest UI/chronicle classification label (`QD-GENESIS`).
    pub const fn display_label(self) -> &'static str {
        match self {
            Self::G1 => "designed founders",
            Self::G2 => "screened founder pool",
            Self::G3 => "emergent primordial",
        }
    }
}

impl fmt::Display for GenesisPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// Persistent identifier of one shared island world (`QD-PLAYRULES` Rule 6). Zero is invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IslandId(pub u64);

impl IslandId {
    /// Wrap a non-zero raw identifier, returning `None` for `0`.
    pub const fn new(raw: u64) -> Option<Self> {
        if raw == 0 {
            None
        } else {
            Some(Self(raw))
        }
    }

    pub const fn raw(self) -> u64 {
        self.0
    }

    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

/// Persistent identifier of one guest session participant (`QD-PLAYRULES` Rule 6). Zero is invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GuestId(pub u64);

impl GuestId {
    /// Wrap a non-zero raw identifier, returning `None` for `0`.
    pub const fn new(raw: u64) -> Option<Self> {
        if raw == 0 {
            None
        } else {
            Some(Self(raw))
        }
    }

    pub const fn raw(self) -> u64 {
        self.0
    }

    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

/// Declared participation mode for a LAN guest (`QD-SHAREDISLAND`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuestRole {
    /// Read-only observer camera (`no organism lease`, `WorldChecksum` matches Solo).
    Spectate,
    /// Embodied intent control of a single animal or individual plant under a leased [`CauseId`].
    Inhabit,
}

/// Host moderation actions applied at tick boundary (`QD-PLAYRULES` Rule 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum HostModerationAction {
    /// Immediately disconnect/revoke a guest's lease and return any leased organism to AI control.
    KickGuest {
        guest_id: GuestId,
        cause_id: CauseId,
    },
}

/// Declared real-time island ecology pacing parameters (`QD-PLAYRULES`, `D-TIME-1`, `D-TIME-2`,
/// `QD-SCALE-TRIAL`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealTimeEcologyProfile {
    pub sim_ticks_per_second: u32,
    pub day_length_seconds: u32,
    pub year_length_seconds: u32,
    pub creature_lifespan_seconds: u32,
    pub maturation_fraction: f32,
    pub gestation_cooldown_seconds: u32,
    pub target_animal_count: u32,
}

impl Default for RealTimeEcologyProfile {
    fn default() -> Self {
        Self {
            sim_ticks_per_second: DEFAULT_SIM_TICKS_PER_SECOND,
            day_length_seconds: DEFAULT_DAY_LENGTH_SECONDS,
            year_length_seconds: DEFAULT_YEAR_LENGTH_SECONDS,
            creature_lifespan_seconds: DEFAULT_CREATURE_LIFESPAN_SECONDS,
            maturation_fraction: DEFAULT_MATURATION_FRACTION,
            gestation_cooldown_seconds: DEFAULT_GESTATION_COOLDOWN_SECONDS,
            target_animal_count: DEFAULT_ISLAND_TARGET_ANIMALS,
        }
    }
}

impl RealTimeEcologyProfile {
    pub const fn day_length_ticks(&self) -> u64 {
        self.day_length_seconds as u64 * self.sim_ticks_per_second as u64
    }

    pub const fn year_length_ticks(&self) -> u64 {
        self.year_length_seconds as u64 * self.sim_ticks_per_second as u64
    }

    pub const fn creature_lifespan_ticks(&self) -> u64 {
        self.creature_lifespan_seconds as u64 * self.sim_ticks_per_second as u64
    }

    pub fn maturation_ticks(&self) -> u64 {
        ((self.creature_lifespan_ticks() as f64) * f64::from(self.maturation_fraction)).round()
            as u64
    }

    pub const fn gestation_cooldown_ticks(&self) -> u64 {
        self.gestation_cooldown_seconds as u64 * self.sim_ticks_per_second as u64
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.sim_ticks_per_second == 0 || self.sim_ticks_per_second > 120 {
            return Err(format!(
                "sim_ticks_per_second {} must lie in 1..=120",
                self.sim_ticks_per_second
            ));
        }
        if self.day_length_seconds == 0 {
            return Err("day_length_seconds must be > 0".into());
        }
        if self.year_length_seconds < self.day_length_seconds {
            return Err(format!(
                "year_length_seconds ({}) must be >= day_length_seconds ({})",
                self.year_length_seconds, self.day_length_seconds
            ));
        }
        if self.creature_lifespan_seconds == 0 {
            return Err("creature_lifespan_seconds must be > 0".into());
        }
        if !self.maturation_fraction.is_finite()
            || self.maturation_fraction <= 0.0
            || self.maturation_fraction >= 1.0
        {
            return Err(format!(
                "maturation_fraction ({}) must be finite and in (0, 1)",
                self.maturation_fraction
            ));
        }
        if self.gestation_cooldown_seconds == 0 {
            return Err("gestation_cooldown_seconds must be > 0".into());
        }
        if self.target_animal_count == 0 {
            return Err("target_animal_count must be > 0".into());
        }
        Ok(())
    }

    pub fn canonicalize(&self, c: &mut Canon) {
        c.u32(self.sim_ticks_per_second)
            .u32(self.day_length_seconds)
            .u32(self.year_length_seconds)
            .u32(self.creature_lifespan_seconds)
            .f32(self.maturation_fraction)
            .u32(self.gestation_cooldown_seconds)
            .u32(self.target_animal_count);
    }
}

/// Optional shared-island declaration carried on [`ExperimentManifest::island`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IslandManifest {
    pub island_id: IslandId,
    pub session_enabled: bool,
    pub max_guests: u16,
    pub genesis_phase: GenesisPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_time_profile: Option<RealTimeEcologyProfile>,
    #[serde(default = "default_true")]
    pub faster_than_real_time_only_when_empty: bool,
}

impl IslandManifest {
    /// Construct a valid Milestone 1 shared-island declaration with `QD-SCALE-TRIAL`, `D-TIME-1/2`,
    /// and `G1` designed-founder defaults.
    pub fn trial_default(island_id: IslandId) -> Self {
        Self {
            island_id,
            session_enabled: true,
            max_guests: DEFAULT_ISLAND_MAX_GUESTS,
            genesis_phase: GenesisPhase::G1,
            real_time_profile: Some(RealTimeEcologyProfile::default()),
            faster_than_real_time_only_when_empty: true,
        }
    }

    /// Validate shared-island manifest configuration against [`ObserverPolicy`] and
    /// [`WorldIdentity`] (`QD-SHAREDISLAND`, `QD-GENESIS`, `QD-PLAYRULES`).
    pub fn validate(
        &self,
        observer: ObserverPolicy,
        world_identity: &WorldIdentity,
    ) -> Result<(), String> {
        if !self.island_id.is_valid() {
            return Err("island_id must be non-zero".into());
        }
        if self.max_guests > MAX_ISLAND_GUESTS_CEILING {
            return Err(format!(
                "max_guests ({}) exceeds host ceiling {}",
                self.max_guests, MAX_ISLAND_GUESTS_CEILING
            ));
        }
        if self.session_enabled {
            if !observer.allows_island_guests() {
                return Err(
                    "shared-island guest session requires ObserverPolicy::Spectate or Inhabit; \
                     ObserverPolicy::Absent refuses island guest ingress (ADR-0004 / ADR-0014)"
                        .into(),
                );
            }
            if self.max_guests == 0 {
                return Err("session_enabled requires max_guests >= 1".into());
            }
            if !self.faster_than_real_time_only_when_empty {
                return Err(
                    "faster_than_real_time_only_when_empty must remain true when guest sessions \
                     are enabled (QD-PLAYRULES Rule 1)"
                        .into(),
                );
            }
        }
        if let Some(profile) = &self.real_time_profile {
            profile.validate()?;
        }
        if let Some(world_phase) = world_identity.genesis_phase {
            if world_phase != self.genesis_phase {
                return Err(format!(
                    "island.genesis_phase ({}) contradicts world_identity.genesis_phase ({world_phase})",
                    self.genesis_phase
                ));
            }
        }
        Ok(())
    }

    /// Append the canonical identity bytes of this declaration to `c`.
    pub fn canonicalize(&self, c: &mut Canon) {
        c.tag(0xF9)
            .u64(self.island_id.raw())
            .bool(self.session_enabled)
            .u16(self.max_guests)
            .tag(self.genesis_phase.canon_tag())
            .bool(self.faster_than_real_time_only_when_empty);
        match &self.real_time_profile {
            None => {
                c.tag(0x00);
            }
            Some(profile) => {
                c.tag(0x01);
                profile.canonicalize(c);
            }
        }
    }
}

/// Effective organism genesis phase declared by a manifest (`island.genesis_phase` or
/// `world_identity.genesis_phase`).
pub fn effective_genesis_phase(manifest: &ExperimentManifest) -> Option<GenesisPhase> {
    manifest
        .island
        .as_ref()
        .map(|island| island.genesis_phase)
        .or(manifest.world_identity.genesis_phase)
}

#[derive(Clone, Copy, Debug)]
struct GuestSlot {
    guest_id: GuestId,
    role: GuestRole,
    cause_id: CauseId,
}

/// Host-side guest admission policy and per-guest reserved `CauseId` table (`ADR-0014`).
///
/// Pre-allocates its slot table up front and does not allocate on steady-state look-ups or
/// moderation revocations.
#[derive(Resource, Clone, Debug)]
pub struct IslandGuestPolicy {
    island_id: IslandId,
    max_guests: u16,
    slots: Vec<Option<GuestSlot>>,
}

impl IslandGuestPolicy {
    /// Allocate a fixed-capacity guest slot table for `island_id` capped at
    /// `MAX_ISLAND_GUESTS_CEILING`.
    pub fn new(island_id: IslandId, max_guests: u16) -> Self {
        let cap = max_guests.min(MAX_ISLAND_GUESTS_CEILING);
        Self {
            island_id,
            max_guests: cap,
            slots: vec![None; cap as usize],
        }
    }

    pub const fn island_id(&self) -> IslandId {
        self.island_id
    }

    pub const fn max_guests(&self) -> u16 {
        self.max_guests
    }

    /// Admit or refresh a guest session under `observer_policy` and return its reserved `CauseId`.
    pub fn admit_guest(
        &mut self,
        guest_id: GuestId,
        role: GuestRole,
        observer_policy: ObserverPolicy,
    ) -> Result<CauseId, GuestIntentDecodeError> {
        if !guest_id.is_valid() {
            return Err(GuestIntentDecodeError::InvalidGuestId);
        }
        if !observer_policy.allows_guest_role(role) {
            return Err(GuestIntentDecodeError::RoleForbidden);
        }
        for existing in self.slots.iter_mut().flatten() {
            if existing.guest_id == guest_id {
                existing.role = role;
                return Ok(existing.cause_id);
            }
        }
        for (idx, slot) in self.slots.iter_mut().enumerate() {
            if slot.is_none() {
                let slot_idx =
                    u16::try_from(idx).map_err(|_| GuestIntentDecodeError::InvalidCauseId)?;
                let cause_id = island_guest_cause_id(slot_idx)
                    .ok_or(GuestIntentDecodeError::InvalidCauseId)?;
                *slot = Some(GuestSlot {
                    guest_id,
                    role,
                    cause_id,
                });
                return Ok(cause_id);
            }
        }
        Err(GuestIntentDecodeError::QueueFullThrottled)
    }

    /// Revoke an active guest (disconnect or host kick) and return the boundary moderation action.
    pub fn kick_guest(&mut self, guest_id: GuestId) -> Option<HostModerationAction> {
        for slot in &mut self.slots {
            if let Some(existing) = slot {
                if existing.guest_id == guest_id {
                    let cause_id = existing.cause_id;
                    *slot = None;
                    return Some(HostModerationAction::KickGuest { guest_id, cause_id });
                }
            }
        }
        None
    }

    pub fn role_and_cause(&self, guest_id: GuestId) -> Option<(GuestRole, CauseId)> {
        self.slots
            .iter()
            .flatten()
            .find(|s| s.guest_id == guest_id)
            .map(|s| (s.role, s.cause_id))
    }

    pub fn active_guest_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    /// `QD-PLAYRULES` Rule 1: faster-than-real-time play is allowed ONLY when zero guests are connected.
    pub fn faster_than_real_time_allowed(&self) -> bool {
        self.active_guest_count() == 0
    }
}
