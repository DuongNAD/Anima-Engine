//! Pre-allocated fixed-capacity SPSC ring buffer and `Copy` wire types for WR2 (ADR-0008 H2).
//!
//! Nothing handed across this boundary allocates on the heap: all strings and trait lists fit in
//! stack-allocated `Copy` inline buffers (`InlineStr`, `CompactIntervention`), and `RingProducer`
//! never blocks or grows the backing array.

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::core::causal::CauseId;
use crate::core::intervention::{Curve, InterventionCommand, InterventionKind, Region, TraitMod};
use crate::core::observer::ObserverAction;

use super::types::{
    DeathCauseTag, InputKind, RecordedEvent, RecordedInput, WorldEvent, WorldKeyframe,
};

/// Inline stack string capacity for agent lineage IDs inside `RingEntry` (`"live-founder-0000"`, etc.).
pub const INLINE_ID_LEN: usize = 48;

/// Inline stack string capacity for chronicle/shard short labels inside `RingEntry`.
pub const INLINE_TEXT_LEN: usize = 64;

/// Maximum `TraitMod` entries stored inline for `InterventionKind::GeneticInjection`.
pub const MAX_COMPACT_TRAIT_MODS: usize = 8;

/// Stack-allocated UTF-8 string (`Copy`, zero heap allocations).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InlineStr<const N: usize> {
    len: u8,
    bytes: [u8; N],
}

impl<const N: usize> Default for InlineStr<N> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<const N: usize> InlineStr<N> {
    /// Construct an empty inline string.
    pub const fn empty() -> Self {
        Self {
            len: 0,
            bytes: [0u8; N],
        }
    }

    /// Copy `s` without allocating, truncating at a valid UTF-8 boundary within `N.min(255)` bytes.
    pub fn from_str_truncating(s: &str) -> Self {
        let cap = N.min(u8::MAX as usize);
        let mut end = s.len().min(cap);
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        let mut bytes = [0u8; N];
        bytes[..end].copy_from_slice(&s.as_bytes()[..end]);
        Self {
            len: end as u8,
            bytes,
        }
    }

    /// Format `"entity-{idx}"` on the stack with zero heap allocations.
    pub fn from_entity_index(idx: u32) -> Self {
        let cap = N.min(u8::MAX as usize);
        let prefix = b"entity-";
        let mut digits = [0u8; 10];
        let mut dlen = 0usize;
        let mut n = idx;
        if n == 0 {
            digits[0] = b'0';
            dlen = 1;
        } else {
            while n > 0 {
                digits[dlen] = b'0' + (n % 10) as u8;
                n /= 10;
                dlen += 1;
            }
        }
        let mut bytes = [0u8; N];
        let mut pos = 0usize;
        let p_take = prefix.len().min(cap);
        bytes[..p_take].copy_from_slice(&prefix[..p_take]);
        pos += p_take;
        while dlen > 0 && pos < cap {
            dlen -= 1;
            bytes[pos] = digits[dlen];
            pos += 1;
        }
        Self {
            len: pos as u8,
            bytes,
        }
    }

    /// Borrow stored UTF-8 slice.
    pub fn as_str(&self) -> &str {
        let end = (self.len as usize).min(N);
        std::str::from_utf8(&self.bytes[..end]).unwrap_or("")
    }

    /// Convert to owned `String` (intended for the background writer thread only).
    pub fn to_owned_string(&self) -> String {
        self.as_str().to_string()
    }
}

/// Stack-allocated mirror of [`TraitMod`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CompactTraitMod {
    ScaleMorphology(f32),
    MassMultiplier(f32),
    MetabolicEfficiencyDelta(f32),
    SensoryRangeMultiplier(f32),
    BrainParameterShift { layer_index: u32, bias_delta: f32 },
}

impl From<&TraitMod> for CompactTraitMod {
    fn from(m: &TraitMod) -> Self {
        match *m {
            TraitMod::ScaleMorphology(v) => Self::ScaleMorphology(v),
            TraitMod::MassMultiplier(v) => Self::MassMultiplier(v),
            TraitMod::MetabolicEfficiencyDelta(v) => Self::MetabolicEfficiencyDelta(v),
            TraitMod::SensoryRangeMultiplier(v) => Self::SensoryRangeMultiplier(v),
            TraitMod::BrainParameterShift {
                layer_index,
                bias_delta,
            } => Self::BrainParameterShift {
                layer_index,
                bias_delta,
            },
        }
    }
}

impl CompactTraitMod {
    fn to_trait_mod(self) -> TraitMod {
        match self {
            Self::ScaleMorphology(v) => TraitMod::ScaleMorphology(v),
            Self::MassMultiplier(v) => TraitMod::MassMultiplier(v),
            Self::MetabolicEfficiencyDelta(v) => TraitMod::MetabolicEfficiencyDelta(v),
            Self::SensoryRangeMultiplier(v) => TraitMod::SensoryRangeMultiplier(v),
            Self::BrainParameterShift {
                layer_index,
                bias_delta,
            } => TraitMod::BrainParameterShift {
                layer_index,
                bias_delta,
            },
        }
    }
}

/// Stack-allocated mirror of [`InterventionKind`] (`Copy`, zero heap allocations).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CompactInterventionKind {
    RainfallDelta,
    TemperatureDelta,
    Deforest,
    RemovePredators,
    AddNutrient,
    GeneticInjection {
        target_species: Option<u32>,
        trait_mods: [CompactTraitMod; MAX_COMPACT_TRAIT_MODS],
        trait_mod_len: u8,
        count: usize,
    },
    ToxicityPlume {
        center_x: f32,
        center_y: f32,
        radius: f32,
        toxicity_level: f32,
        decay_rate: f32,
    },
    PathogenTransmission {
        pathogen_id: u32,
        transmission_rate: f32,
        mortality_multiplier: f32,
    },
}

/// Stack-allocated mirror of [`InterventionCommand`] (`Copy`, zero heap allocations).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompactIntervention {
    pub id: u32,
    pub cause_id: CauseId,
    pub kind: CompactInterventionKind,
    pub region: Region,
    pub start_tick: u64,
    pub duration_ticks: u64,
    pub intensity: f32,
    pub signed_negative: bool,
    pub curve: Curve,
    pub reversible: bool,
}

impl CompactIntervention {
    /// Convert a live [`InterventionCommand`] into a `Copy` representation without allocating.
    pub fn from_command(cmd: &InterventionCommand) -> Self {
        let kind = match &cmd.kind {
            InterventionKind::RainfallDelta => CompactInterventionKind::RainfallDelta,
            InterventionKind::TemperatureDelta => CompactInterventionKind::TemperatureDelta,
            InterventionKind::Deforest => CompactInterventionKind::Deforest,
            InterventionKind::RemovePredators => CompactInterventionKind::RemovePredators,
            InterventionKind::AddNutrient => CompactInterventionKind::AddNutrient,
            InterventionKind::GeneticInjection {
                target_species,
                trait_modifications,
                count,
            } => {
                let mut mods = [CompactTraitMod::ScaleMorphology(1.0); MAX_COMPACT_TRAIT_MODS];
                let len = trait_modifications.len().min(MAX_COMPACT_TRAIT_MODS);
                for (slot, src) in mods.iter_mut().zip(trait_modifications.iter().take(len)) {
                    *slot = CompactTraitMod::from(src);
                }
                CompactInterventionKind::GeneticInjection {
                    target_species: *target_species,
                    trait_mods: mods,
                    trait_mod_len: len as u8,
                    count: *count,
                }
            }
            InterventionKind::ToxicityPlume {
                center_x,
                center_y,
                radius,
                toxicity_level,
                decay_rate,
            } => CompactInterventionKind::ToxicityPlume {
                center_x: *center_x,
                center_y: *center_y,
                radius: *radius,
                toxicity_level: *toxicity_level,
                decay_rate: *decay_rate,
            },
            InterventionKind::PathogenTransmission {
                pathogen_id,
                transmission_rate,
                mortality_multiplier,
            } => CompactInterventionKind::PathogenTransmission {
                pathogen_id: *pathogen_id,
                transmission_rate: *transmission_rate,
                mortality_multiplier: *mortality_multiplier,
            },
        };
        Self {
            id: cmd.id,
            cause_id: cmd.cause_id,
            kind,
            region: cmd.region,
            start_tick: cmd.start_tick,
            duration_ticks: cmd.duration_ticks,
            intensity: cmd.intensity,
            signed_negative: cmd.signed_negative,
            curve: cmd.curve,
            reversible: cmd.reversible,
        }
    }

    /// Reconstruct owned [`InterventionCommand`] off the hot simulation thread.
    pub fn to_command(&self) -> InterventionCommand {
        let kind = match self.kind {
            CompactInterventionKind::RainfallDelta => InterventionKind::RainfallDelta,
            CompactInterventionKind::TemperatureDelta => InterventionKind::TemperatureDelta,
            CompactInterventionKind::Deforest => InterventionKind::Deforest,
            CompactInterventionKind::RemovePredators => InterventionKind::RemovePredators,
            CompactInterventionKind::AddNutrient => InterventionKind::AddNutrient,
            CompactInterventionKind::GeneticInjection {
                target_species,
                trait_mods,
                trait_mod_len,
                count,
            } => {
                let len = (trait_mod_len as usize).min(MAX_COMPACT_TRAIT_MODS);
                InterventionKind::GeneticInjection {
                    target_species,
                    trait_modifications: trait_mods[..len]
                        .iter()
                        .map(|m| m.to_trait_mod())
                        .collect(),
                    count,
                }
            }
            CompactInterventionKind::ToxicityPlume {
                center_x,
                center_y,
                radius,
                toxicity_level,
                decay_rate,
            } => InterventionKind::ToxicityPlume {
                center_x,
                center_y,
                radius,
                toxicity_level,
                decay_rate,
            },
            CompactInterventionKind::PathogenTransmission {
                pathogen_id,
                transmission_rate,
                mortality_multiplier,
            } => InterventionKind::PathogenTransmission {
                pathogen_id,
                transmission_rate,
                mortality_multiplier,
            },
        };
        InterventionCommand {
            id: self.id,
            cause_id: self.cause_id,
            kind,
            region: self.region,
            start_tick: self.start_tick,
            duration_ticks: self.duration_ticks,
            intensity: self.intensity,
            signed_negative: self.signed_negative,
            curve: self.curve,
            reversible: self.reversible,
        }
    }
}

/// Fixed-size (`Copy`), allocation-free entry transported through the SPSC ring buffer.
#[derive(Clone, Copy, Debug)]
pub enum RingEntry {
    InputIntervention {
        tick: u64,
        cause_id: CauseId,
        command: CompactIntervention,
    },
    InputObserver {
        tick: u64,
        cause_id: CauseId,
        action: ObserverAction,
    },
    InputChronicle {
        tick: u64,
        cause_id: CauseId,
        event_type: InlineStr<32>,
        title: InlineStr<INLINE_TEXT_LEN>,
        description: InlineStr<INLINE_TEXT_LEN>,
    },
    EventBirth {
        tick: u64,
        agent_id: InlineStr<INLINE_ID_LEN>,
        parent_id: Option<InlineStr<INLINE_ID_LEN>>,
        generation: u32,
        energy_transferred: f64,
    },
    EventDeath {
        tick: u64,
        agent_id: InlineStr<INLINE_ID_LEN>,
        cause: DeathCauseTag,
        age_ticks: u64,
    },
    EventKill {
        tick: u64,
        predator_index: u32,
        prey_index: u32,
        damage: f64,
        energy_transferred: f64,
    },
    EventMigration {
        tick: u64,
        agent_id: InlineStr<INLINE_ID_LEN>,
        from_shard: InlineStr<32>,
        to_shard: InlineStr<32>,
    },
    EventChronicle {
        tick: u64,
        category: InlineStr<32>,
        summary: InlineStr<INLINE_TEXT_LEN>,
    },
    EventEpoch {
        tick: u64,
        epoch: u32,
        replaced_count: u32,
    },
    SegmentSeal {
        tick_start: u64,
        tick_end: u64,
        end_checksum: u32,
        keyframe: WorldKeyframe,
        dropped_inputs_since_prev: u64,
        dropped_events_since_prev: u64,
    },
}

impl RingEntry {
    /// Convert a [`RecordedInput`] into a [`RingEntry`] cleanly when driven via public APIs.
    pub fn from_recorded_input(input: &RecordedInput) -> Self {
        match &input.kind {
            InputKind::Intervention(cmd) => Self::InputIntervention {
                tick: input.tick,
                cause_id: input.cause_id,
                command: CompactIntervention::from_command(cmd),
            },
            InputKind::ObserverAction(action) => Self::InputObserver {
                tick: input.tick,
                cause_id: input.cause_id,
                action: *action,
            },
            InputKind::ChronicleNote {
                event_type,
                title,
                description,
            } => Self::InputChronicle {
                tick: input.tick,
                cause_id: input.cause_id,
                event_type: InlineStr::from_str_truncating(event_type),
                title: InlineStr::from_str_truncating(title),
                description: InlineStr::from_str_truncating(description),
            },
        }
    }

    /// Convert a `(tick, WorldEvent)` into a [`RingEntry`] cleanly when driven via public APIs.
    pub fn from_world_event(tick: u64, event: &WorldEvent) -> Self {
        match event {
            WorldEvent::Birth {
                agent_id,
                parent_id,
                generation,
                energy_transferred,
            } => Self::EventBirth {
                tick,
                agent_id: InlineStr::from_str_truncating(agent_id),
                parent_id: parent_id.as_deref().map(InlineStr::from_str_truncating),
                generation: *generation,
                energy_transferred: *energy_transferred,
            },
            WorldEvent::Death {
                agent_id,
                cause,
                age_ticks,
            } => Self::EventDeath {
                tick,
                agent_id: InlineStr::from_str_truncating(agent_id),
                cause: *cause,
                age_ticks: *age_ticks,
            },
            WorldEvent::Kill {
                predator_id,
                prey_id,
                damage,
                energy_transferred,
            } => {
                let pred = predator_id
                    .strip_prefix("entity-")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);
                let prey = prey_id
                    .strip_prefix("entity-")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);
                Self::EventKill {
                    tick,
                    predator_index: pred,
                    prey_index: prey,
                    damage: *damage,
                    energy_transferred: *energy_transferred,
                }
            }
            WorldEvent::Migration {
                agent_id,
                from_shard,
                to_shard,
            } => Self::EventMigration {
                tick,
                agent_id: InlineStr::from_str_truncating(agent_id),
                from_shard: InlineStr::from_str_truncating(from_shard),
                to_shard: InlineStr::from_str_truncating(to_shard),
            },
            WorldEvent::Chronicle { category, summary } => Self::EventChronicle {
                tick,
                category: InlineStr::from_str_truncating(category),
                summary: InlineStr::from_str_truncating(summary),
            },
            WorldEvent::EpochReplacement {
                epoch,
                replaced_count,
            } => Self::EventEpoch {
                tick,
                epoch: *epoch,
                replaced_count: *replaced_count,
            },
        }
    }

    /// Reconstruct [`RecordedInput`] off the hot thread if this entry holds an input.
    pub fn into_recorded_input(self) -> Option<RecordedInput> {
        match self {
            Self::InputIntervention {
                tick,
                cause_id,
                command,
            } => Some(RecordedInput {
                tick,
                cause_id,
                kind: InputKind::Intervention(command.to_command()),
            }),
            Self::InputObserver {
                tick,
                cause_id,
                action,
            } => Some(RecordedInput {
                tick,
                cause_id,
                kind: InputKind::ObserverAction(action),
            }),
            Self::InputChronicle {
                tick,
                cause_id,
                event_type,
                title,
                description,
            } => Some(RecordedInput {
                tick,
                cause_id,
                kind: InputKind::ChronicleNote {
                    event_type: event_type.to_owned_string(),
                    title: title.to_owned_string(),
                    description: description.to_owned_string(),
                },
            }),
            _ => None,
        }
    }

    /// Reconstruct [`RecordedEvent`] off the hot thread if this entry holds an event.
    pub fn into_recorded_event(self) -> Option<RecordedEvent> {
        match self {
            Self::EventBirth {
                tick,
                agent_id,
                parent_id,
                generation,
                energy_transferred,
            } => Some(RecordedEvent {
                tick,
                kind: WorldEvent::Birth {
                    agent_id: agent_id.to_owned_string(),
                    parent_id: parent_id.map(|p| p.to_owned_string()),
                    generation,
                    energy_transferred,
                },
            }),
            Self::EventDeath {
                tick,
                agent_id,
                cause,
                age_ticks,
            } => Some(RecordedEvent {
                tick,
                kind: WorldEvent::Death {
                    agent_id: agent_id.to_owned_string(),
                    cause,
                    age_ticks,
                },
            }),
            Self::EventKill {
                tick,
                predator_index,
                prey_index,
                damage,
                energy_transferred,
            } => Some(RecordedEvent {
                tick,
                kind: WorldEvent::Kill {
                    predator_id: format!("entity-{predator_index}"),
                    prey_id: format!("entity-{prey_index}"),
                    damage,
                    energy_transferred,
                },
            }),
            Self::EventMigration {
                tick,
                agent_id,
                from_shard,
                to_shard,
            } => Some(RecordedEvent {
                tick,
                kind: WorldEvent::Migration {
                    agent_id: agent_id.to_owned_string(),
                    from_shard: from_shard.to_owned_string(),
                    to_shard: to_shard.to_owned_string(),
                },
            }),
            Self::EventChronicle {
                tick,
                category,
                summary,
            } => Some(RecordedEvent {
                tick,
                kind: WorldEvent::Chronicle {
                    category: category.to_owned_string(),
                    summary: summary.to_owned_string(),
                },
            }),
            Self::EventEpoch {
                tick,
                epoch,
                replaced_count,
            } => Some(RecordedEvent {
                tick,
                kind: WorldEvent::EpochReplacement {
                    epoch,
                    replaced_count,
                },
            }),
            _ => None,
        }
    }
}

struct RingCore<T: Copy> {
    slots: Box<[UnsafeCell<MaybeUninit<T>>]>,
    mask: usize,
    logical_capacity: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
}

// Sound for single-producer (`RingProducer`) and single-consumer (`RingConsumer`) because
// producer only mutates `slots[tail & mask]` when `tail - head < logical_capacity`, and
// consumer only reads `slots[head & mask]` after observing `tail` published with Release/Acquire.
unsafe impl<T: Copy + Send> Send for RingCore<T> {}
unsafe impl<T: Copy + Send> Sync for RingCore<T> {}

/// Single-producer non-blocking handle to a pre-allocated SPSC ring buffer.
pub struct RingProducer<T: Copy> {
    core: Arc<RingCore<T>>,
}

/// Single-consumer non-blocking handle to a pre-allocated SPSC ring buffer.
pub struct RingConsumer<T: Copy> {
    core: Arc<RingCore<T>>,
}

/// Allocate a wait-free, fixed-capacity SPSC ring buffer storing up to `logical_capacity.max(2)` items.
pub fn spsc_ring<T: Copy>(logical_capacity: usize) -> (RingProducer<T>, RingConsumer<T>) {
    let logical_capacity = logical_capacity.max(2);
    let buf_len = logical_capacity.next_power_of_two();
    let slots: Vec<UnsafeCell<MaybeUninit<T>>> = (0..buf_len)
        .map(|_| UnsafeCell::new(MaybeUninit::uninit()))
        .collect();
    let core = Arc::new(RingCore {
        slots: slots.into_boxed_slice(),
        mask: buf_len - 1,
        logical_capacity,
        head: AtomicUsize::new(0),
        tail: AtomicUsize::new(0),
    });
    (
        RingProducer {
            core: Arc::clone(&core),
        },
        RingConsumer { core },
    )
}

impl<T: Copy> RingProducer<T> {
    /// Fixed maximum number of queued items before `try_push` returns `Err(item)`.
    pub fn logical_capacity(&self) -> usize {
        self.core.logical_capacity
    }

    /// Conservative snapshot of remaining writable slots without blocking.
    pub fn available_slots(&self) -> usize {
        let head = self.core.head.load(Ordering::Acquire);
        let tail = self.core.tail.load(Ordering::Relaxed);
        let used = tail.wrapping_sub(head);
        self.core.logical_capacity.saturating_sub(used)
    }

    /// Attempt to push `item` without blocking or allocating; returns `Err(item)` when full.
    pub fn try_push(&mut self, item: T) -> Result<(), T> {
        let head = self.core.head.load(Ordering::Acquire);
        let tail = self.core.tail.load(Ordering::Relaxed);
        if tail.wrapping_sub(head) >= self.core.logical_capacity {
            return Err(item);
        }
        let idx = tail & self.core.mask;
        // Exclusive access guaranteed because `tail - head < logical_capacity <= slots.len()` and only
        // this producer mutates `slots[idx]` while `tail` has not yet been advanced past it.
        unsafe {
            (*self.core.slots[idx].get()).write(item);
        }
        self.core
            .tail
            .store(tail.wrapping_add(1), Ordering::Release);
        Ok(())
    }
}

impl<T: Copy> RingConsumer<T> {
    /// Attempt to pop the oldest queued item; returns `None` when empty.
    pub fn try_pop(&mut self) -> Option<T> {
        let tail = self.core.tail.load(Ordering::Acquire);
        let head = self.core.head.load(Ordering::Relaxed);
        if head == tail {
            return None;
        }
        let idx = head & self.core.mask;
        // Safe to read because producer wrote before publishing `tail` with `Ordering::Release`.
        let item = unsafe { (*self.core.slots[idx].get()).assume_init_read() };
        self.core
            .head
            .store(head.wrapping_add(1), Ordering::Release);
        Some(item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_str_truncates_utf8_cleanly_and_formats_entity_indices() {
        let ascii: InlineStr<8> = InlineStr::from_str_truncating("live-founder-0001");
        assert_eq!(ascii.as_str(), "live-fou");

        // 2-byte UTF-8 chars (`é`) must never split mid-codepoint.
        let utf8: InlineStr<5> = InlineStr::from_str_truncating("ééé");
        assert_eq!(utf8.as_str(), "éé");

        let e0: InlineStr<INLINE_ID_LEN> = InlineStr::from_entity_index(0);
        let e42: InlineStr<INLINE_ID_LEN> = InlineStr::from_entity_index(42);
        let e_max: InlineStr<INLINE_ID_LEN> = InlineStr::from_entity_index(u32::MAX);
        assert_eq!(e0.as_str(), "entity-0");
        assert_eq!(e42.as_str(), "entity-42");
        assert_eq!(e_max.as_str(), format!("entity-{}", u32::MAX));
    }

    #[test]
    fn spsc_ring_enforces_exact_capacity_and_wraparound_fifo() {
        let (mut tx, mut rx) = spsc_ring::<u32>(4);
        assert_eq!(tx.logical_capacity(), 4);
        assert_eq!(tx.available_slots(), 4);

        for v in 1..=4 {
            assert_eq!(tx.try_push(v), Ok(()));
        }
        assert_eq!(tx.available_slots(), 0);
        assert_eq!(tx.try_push(5), Err(5));

        assert_eq!(rx.try_pop(), Some(1));
        assert_eq!(rx.try_pop(), Some(2));
        assert_eq!(tx.try_push(5), Ok(()));
        assert_eq!(tx.try_push(6), Ok(()));
        assert_eq!(tx.try_push(7), Err(7));

        let mut drained = Vec::new();
        while let Some(v) = rx.try_pop() {
            drained.push(v);
        }
        assert_eq!(drained, vec![3, 4, 5, 6]);
        assert_eq!(rx.try_pop(), None);
    }

    #[test]
    fn compact_intervention_round_trips_all_variants() {
        let cmd = InterventionCommand {
            id: 9,
            cause_id: 3,
            kind: InterventionKind::GeneticInjection {
                target_species: Some(2),
                trait_modifications: vec![
                    TraitMod::ScaleMorphology(1.1),
                    TraitMod::BrainParameterShift {
                        layer_index: 1,
                        bias_delta: -0.05,
                    },
                ],
                count: 5,
            },
            region: Region::Radius {
                cx: 4.0,
                cy: 5.0,
                r: 2.5,
            },
            start_tick: 10,
            duration_ticks: 30,
            intensity: 0.75,
            signed_negative: true,
            curve: Curve::Triangle,
            reversible: false,
        };
        let compact = CompactIntervention::from_command(&cmd);
        assert_eq!(compact.to_command(), cmd);
    }
}
