//! Time laws: the constants that define how fast simulated time runs.
//!
//! These moved out of `core::sim_rules` because they are laws, not engine settings — the headless
//! runner and the live Bevy world must agree on them or a manifest replayed in one does not
//! describe the other. `core::sim_rules` re-exports them, so the units table and the S01 machine
//! checks still read as one document.

/// Physics/collision tick rate in Hz — the fixed simulation step. Mirrors `TimeStep(1.0/60.0)`
/// inserted in `core/ecs.rs`.
pub const TICK_HZ: f64 = 60.0;

/// Fixed simulation time step, in sim-seconds (`1 / TICK_HZ` ≈ 0.016667).
pub const TICK_DT_SECONDS: f64 = 1.0 / TICK_HZ;

/// Ticks in one evolutionary epoch (`EpochManager::ticks_per_epoch`, `core/simulation_loop.rs`).
pub const TICKS_PER_EPOCH: u64 = 1000;

/// Sim-seconds in one simulated year: the `SeasonClock` sweeps one full 2π phase over this span
/// (`rate = TAU / SECONDS_PER_YEAR` in `core/ecology.rs`).
pub const SECONDS_PER_YEAR: f64 = 100.0;

/// Ticks in one simulated year (`SECONDS_PER_YEAR * TICK_HZ` = 6000 at 60 Hz).
pub const TICKS_PER_YEAR: u64 = 6000;

// ---- Biophysical & Ecological Laws (MTE & Functional Responses) ----

/// Boltzmann constant in eV/K.
pub const BOLTZMANN_EV: f64 = 8.617_333_262e-5;

/// Reference temperature (20 °C) at which the Arrhenius factor is exactly 1.0.
pub const REF_TEMP_K: f64 = 293.15;

/// Activation energy of respiratory metabolism in animals (Brown et al. 2004: ~0.65 eV).
pub const E_ANIMAL_EV: f64 = 0.65;

/// Activation energy of photosynthetic primary production (~0.30 eV).
pub const E_PRODUCER_EV: f64 = 0.30;

/// Kleiber allometric exponent (3/4 power law).
pub const KLEIBER_EXP: f64 = 0.75;

/// Normalization constant for basal metabolic rate.
pub const METABOLIC_NORM: f64 = 0.06;

/// Canonical Lindeman trophic transfer efficiency (~40% in simulation).
pub const LINDEMAN_EFFICIENCY: f64 = 0.40;

/// Canonical continuous reproduction assimilation efficiency (~60% to offspring, 40% overhead to detritus).
pub const REPRODUCTION_EFFICIENCY: f64 = 0.60;


/// Default Holling Type III attack parameter for predation capture.
pub const CAPTURE_ATTACK: f64 = 0.02;


/// Default Holling Type III handling parameter for predation capture.
pub const CAPTURE_HANDLING: f64 = 0.02;


/// Arrhenius/Boltzmann temperature scaling factor, normalized to 1.0 at `REF_TEMP_K`.
/// `temp_c` is organism body temperature in °C.
#[inline]
pub fn boltzmann_factor(temp_c: f64, activation_ev: f64) -> f64 {
    let t_k = temp_c + 273.15;
    if t_k <= 0.0 {
        return 0.0;
    }
    ((activation_ev / BOLTZMANN_EV) * (1.0 / REF_TEMP_K - 1.0 / t_k)).exp()
}

/// Whole-organism basal metabolic rate $B(M, T) = B_0 M^{3/4} \exp\left(\frac{E_a}{k}\left(\frac{1}{T_0} - \frac{1}{T}\right)\right)$.
#[inline]
pub fn mte_basal_metabolism(mass: f64, temp_kelvin: f64, basal_rate: f64, act_energy: f64) -> f64 {
    if mass <= 0.0 || temp_kelvin <= 0.0 {
        return 0.0;
    }
    let arrhenius = ((act_energy / BOLTZMANN_EV) * (1.0 / REF_TEMP_K - 1.0 / temp_kelvin)).exp();
    basal_rate * mass.powf(KLEIBER_EXP) * arrhenius
}

/// Holling Type II functional response (hyperbolic saturation): $f_{II}(N) = \frac{a N}{1 + a h N}$.
#[inline]
pub fn holling_type_2(prey_density: f64, attack_rate: f64, handling_time: f64) -> f64 {
    let n = prey_density.max(0.0);
    (attack_rate * n) / (1.0 + attack_rate * handling_time * n)
}

/// Holling Type III functional response with rarity refuge: $f_{III}(N) = \frac{a N^2}{1 + a h N^2 + \epsilon_{\text{refuge}}}$.
#[inline]
pub fn holling_type_3(prey_density: f64, attack_rate: f64, handling_time: f64, refuge_param: f64) -> f64 {
    let n = prey_density.max(0.0);
    let n2 = n * n;
    (attack_rate * n2) / (1.0 + attack_rate * handling_time * n2 + refuge_param.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_laws() {
        assert!((TICK_DT_SECONDS - 1.0 / 60.0).abs() < 1e-12);
        assert_eq!(TICKS_PER_YEAR, 6000);
    }

    #[test]
    fn test_mte_scaling() {
        let b0 = mte_basal_metabolism(1.0, REF_TEMP_K, METABOLIC_NORM, E_ANIMAL_EV);
        assert!((b0 - METABOLIC_NORM).abs() < 1e-9);

        // Doubling mass -> 2^0.75 ~= 1.6817928
        let b2 = mte_basal_metabolism(2.0, REF_TEMP_K, METABOLIC_NORM, E_ANIMAL_EV);
        assert!((b2 / b0 - 2.0f64.powf(0.75)).abs() < 1e-6);

        // Q10 temperature check
        let b_warm = mte_basal_metabolism(1.0, REF_TEMP_K + 10.0, METABOLIC_NORM, E_ANIMAL_EV);
        let q10 = b_warm / b0;
        assert!(q10 > 2.0 && q10 < 3.0);
    }

    #[test]
    fn test_holling_functional_responses() {
        // Holling II saturates to 1/h
        let h2_sat = holling_type_2(1e8, 1.0, 0.2);
        assert!((h2_sat - 5.0).abs() < 1e-3);

        // Holling III vanishes quadratically at low density
        let low = 0.1;
        let h2_low = holling_type_2(low, 1.0, 0.5) / low;
        let h3_low = holling_type_3(low, 1.0, 0.5, 0.0) / low;
        assert!(h3_low < h2_low * 0.2);
    }
}

