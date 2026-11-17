use super::*;

// ---- AE-202 law validation --------------------------------------------------------------

#[test]
fn law_rejects_eu_unit_and_bad_ranges() {
    // MU is not EU: a law claiming the EU unit is rejected.
    let mut law = ExoticEnergyLaw::mana_uniform(100.0);
    law.unit = UnitId::new(EU_UNIT);
    assert!(law.validate().unwrap_err().contains("EU"));

    // Negative source rate.
    let mut law = ExoticEnergyLaw::mana_uniform(100.0);
    law.source_rate = -0.1;
    assert!(law.validate().is_err());

    // Unstable diffusion.
    let mut law = ExoticEnergyLaw::mana_uniform(100.0);
    law.diffusion_rate = 0.9;
    assert!(law.validate().unwrap_err().contains("diffusion"));

    // Zero hotspots in a patchy law.
    let mut law = ExoticEnergyLaw::mana_patchy(100.0, 3);
    law.topology = SourceTopology::Patchy {
        hotspot_count: 0,
        radius_cells: 2.0,
    };
    assert!(law.validate().is_err());

    // A well-formed law passes.
    assert!(ExoticEnergyLaw::mana_uniform(100.0).validate().is_ok());
    assert!(ExoticEnergyLaw::mana_patchy(100.0, 4).validate().is_ok());
}

fn build(law: &ExoticEnergyLaw, w: usize, h: usize, seed: u64) -> ExoticEnergyField {
    ExoticEnergyField::from_law(law, w, h, seed).expect("feasible field")
}

// ---- AE-203 field init ------------------------------------------------------------------

#[test]
fn uniform_field_has_the_declared_initial_total_and_is_bounded() {
    let law = ExoticEnergyLaw::mana_uniform(160.0);
    let f = build(&law, 8, 8, 1);
    // Uniform: every cell equal, total ~= initial_amount.
    assert!((f.total() - 160.0).abs() < 1e-3, "total was {}", f.total());
    assert!(f
        .density
        .iter()
        .all(|&v| (0.0..=law.max_density).contains(&v)));
    // All equal (uniform).
    let first = f.density[0];
    assert!(f.density.iter().all(|&v| (v - first).abs() < 1e-4));
}

#[test]
fn patchy_field_is_deterministic_by_seed_and_bounded() {
    let law = ExoticEnergyLaw::mana_patchy(200.0, 5);
    let a = build(&law, 16, 16, 42);
    let b = build(&law, 16, 16, 42);
    // Same seed → identical field (no thread_rng anywhere).
    assert_eq!(a.density, b.density);
    // A different seed places hotspots differently.
    let c = build(&law, 16, 16, 43);
    assert_ne!(a.density, c.density);
    // Bounded and non-negative.
    assert!(a
        .density
        .iter()
        .all(|&v| (0.0..=law.max_density).contains(&v)));
    // Concentrated: not every cell is equal.
    let first = a.density[0];
    assert!(a.density.iter().any(|&v| (v - first).abs() > 1e-4));
}

#[test]
fn defect_c_law_rejects_empty_display_name() {
    let mut law = ExoticEnergyLaw::mana_uniform(100.0);
    law.display_name = String::new();
    assert!(
        law.validate().is_err(),
        "a law with no display name cannot be presented to a user"
    );
}

#[test]
fn defect_c_from_law_validates_defensively_and_rejects_bad_grids() {
    // `from_law` is public, so it must not boot on an invalid law even if a caller skipped
    // `validate()` (defence in depth).
    let mut bad = ExoticEnergyLaw::mana_uniform(10.0);
    bad.unit = UnitId::new(EU_UNIT); // MU is not EU
    assert!(ExoticEnergyField::from_law(&bad, 8, 8, 1).is_err());

    let mut bad = ExoticEnergyLaw::mana_uniform(10.0);
    bad.diffusion_rate = 0.9; // unstable
    assert!(ExoticEnergyField::from_law(&bad, 8, 8, 1).is_err());

    // Zero dimensions must be REJECTED, not silently coerced to a 1x1 grid (which would
    // quietly change the declared world size).
    let good = ExoticEnergyLaw::mana_uniform(10.0);
    assert!(ExoticEnergyField::from_law(&good, 0, 8, 1).is_err());
    assert!(ExoticEnergyField::from_law(&good, 8, 0, 1).is_err());

    // An overflowing grid is rejected rather than panicking / wrapping.
    assert!(ExoticEnergyField::from_law(&good, usize::MAX, 2, 1).is_err());

    // A sane grid still builds.
    assert!(ExoticEnergyField::from_law(&good, 8, 8, 1).is_ok());
}

#[test]
fn over_capacity_initial_amount_is_rejected_not_silently_clamped() {
    // A uniform 4x4 field with max_density 10 holds at most 16*10 = 160 MU. Asking for 400 MU
    // used to be silently clamped (recording a smaller `initial`); it must now be a hard error so
    // the declared initial condition is never quietly changed.
    let mut law = ExoticEnergyLaw::mana_uniform(400.0);
    law.max_density = 10.0;
    let err = ExoticEnergyField::from_law(&law, 4, 4, 1).unwrap_err();
    assert!(err.contains("capacity"), "unexpected error: {err}");

    // At/under capacity is accepted, and the realized total matches the declared amount within
    // f32 rounding (no capacity truncation).
    let mut law = ExoticEnergyLaw::mana_uniform(160.0);
    law.max_density = 10.0;
    let f = ExoticEnergyField::from_law(&law, 4, 4, 1).expect("exactly at capacity is feasible");
    assert!((f.total() - 160.0).abs() / 160.0 < 1e-5);
    assert!((f.initial - 160.0).abs() / 160.0 < 1e-5);

    // Patchy: a single hotspot concentrates MU, so a modest amount can already exceed a cell cap.
    let mut law = ExoticEnergyLaw::mana_patchy(500.0, 1);
    law.max_density = 1.0;
    assert!(ExoticEnergyField::from_law(&law, 8, 8, 5).is_err());
}

// ---- AE-S04 conservation ----------------------------------------------------------------

#[test]
fn closed_diffusion_conserves_mu() {
    // Pure diffusion (no source, no decay), closed boundary → total MU invariant.
    let mut law = ExoticEnergyLaw::mana_patchy(300.0, 6);
    law.source_rate = 0.0;
    law.decay_rate = 0.0;
    law.diffusion_rate = 0.2;
    law.boundary = BoundaryMode::Closed;
    let mut f = build(&law, 24, 24, 7);
    let total0 = f.total();
    for _ in 0..500 {
        f.step();
        assert!(
            (f.total() - total0).abs() / total0 < 1e-4,
            "closed diffusion drifted: {} vs {}",
            f.total(),
            total0
        );
    }
}

#[test]
fn source_decay_diffusion_closes_the_mu_budget() {
    let law = ExoticEnergyLaw::mana_patchy(150.0, 4);
    let mut f = build(&law, 20, 20, 11);
    for _ in 0..800 {
        f.step();
        let b = f.budget(0.0);
        assert!(
            b.balance_error().abs() / b.throughput() < 1e-4,
            "MU budget drifted: error {}, throughput {}",
            b.balance_error(),
            b.throughput()
        );
    }
    // The source and decay actually moved MU.
    let b = f.budget(0.0);
    assert!(b.sourced > 0.0, "renewable source should inject MU");
    assert!(b.dissipated > 0.0, "decay should dissipate MU");
}

#[test]
fn open_boundary_exports_mu_as_a_declared_sink() {
    let mut law = ExoticEnergyLaw::mana_uniform(200.0);
    law.source_rate = 0.0;
    law.decay_rate = 0.0;
    law.diffusion_rate = 0.2;
    law.boundary = BoundaryMode::Open;
    let mut f = build(&law, 12, 12, 3);
    for _ in 0..300 {
        f.step();
        let b = f.budget(0.0);
        assert!(
            b.balance_error().abs() / b.throughput() < 1e-4,
            "open budget must still close via `exported`"
        );
    }
    // Some MU genuinely left the grid.
    assert!(f.cum_exported > 0.0, "open boundary should export MU");
    assert!(f.total() < 200.0);
}

// ---- AE-205 transactions ----------------------------------------------------------------

#[test]
fn uptake_and_spend_conserve_mu_end_to_end() {
    let mut law = ExoticEnergyLaw::mana_uniform(100.0);
    law.source_rate = 0.0;
    law.decay_rate = 0.0;
    law.diffusion_rate = 0.0;
    let mut f = build(&law, 4, 4, 1);
    let mut storage = 0.0f64;
    let mut dissipated_via_spend = 0.0f64;

    // Uptake: field → storage, exactly conserving (field total + storage is invariant to the bit).
    let field_before = f.total();
    let moved = f.uptake(0, 5.0, &mut storage, 1000.0);
    assert!(moved > 0.0);
    assert_eq!(
        f.total() + storage,
        field_before,
        "uptake must conserve field+storage exactly"
    );

    // Spend: storage → dissipated, exactly conserving.
    let stored_before = storage;
    let spent = spend_storage(&mut storage, 2.0, &mut dissipated_via_spend);
    assert!((spent - 2.0).abs() < 1e-9);
    assert!((storage + dissipated_via_spend - stored_before).abs() < 1e-9);

    // Full ledger closes: initial == field + storage + spent-dissipated.
    let b = f.budget(storage);
    let closed = b.initial - (b.field + b.organism_storage) - dissipated_via_spend;
    assert!(
        closed.abs() < 1e-9,
        "end-to-end MU ledger drifted: {closed}"
    );
}

#[test]
fn fractional_and_repeated_uptake_is_ledger_exact() {
    // Awkward, non-representable request amounts drawn repeatedly from the same cell. Storage must
    // gain EXACTLY the field's total decrease (the f32 field delta), never the f64 request — so
    // the running invariant `field_delta == storage` holds to the field's f32 granularity.
    let mut law = ExoticEnergyLaw::mana_uniform(120.0);
    law.source_rate = 0.0;
    law.decay_rate = 0.0;
    law.diffusion_rate = 0.0;
    law.max_density = 100.0; // one cell holds plenty
    let mut f = build(&law, 4, 4, 1);
    let mut storage = 0.0f64;

    let field_before = f.total();
    // Repeated withdrawals of an awkward, non-f32-representable fraction from cell 5.
    for _ in 0..37 {
        f.uptake(5, 0.111_357_9, &mut storage, 1_000_000.0);
    }
    // Documented tolerance: the field is f32, so its total is represented to ~1e-6 relative; the
    // conservation invariant holds to that granularity (and much tighter than the f64 request sum
    // the buggy version would have credited).
    let field_delta = field_before - f.total();
    assert!(
        (field_delta - storage).abs() <= 1e-4,
        "uptake ledger drifted: field lost {field_delta}, storage gained {storage}"
    );
    // The uptake returns the ACTUAL field delta, not the request: 37 × 0.1113579 ≈ 4.12, but the
    // f32 field can only shed representable amounts, so storage ≈ field_delta (proven above), and
    // storage never exceeds the field's real decrease.
    assert!(storage <= field_delta + 1e-9);
}

// ---- AE-209 (M1): exotic-source intervention schema -------------------------------------

fn forcing(id: u32, kind: ExoticInterventionKind, start: u64, dur: u64) -> ExoticIntervention {
    ExoticIntervention {
        id,
        cause_id: 900 + id,
        kind,
        region: Region::Global,
        start_tick: start,
        duration_ticks: dur,
        amount: 1.0,
        curve: Curve::Step,
    }
}

#[test]
fn ae209_m1_forcing_validates_structurally() {
    // Well-formed forcings of each kind pass.
    for kind in [
        ExoticInterventionKind::AddSource,
        ExoticInterventionKind::RemoveSource,
        ExoticInterventionKind::Pulse,
    ] {
        assert!(forcing(1, kind, 10, 100).validate(6000).is_ok(), "{kind:?}");
    }

    // Non-finite / negative amount is rejected (magnitude only; direction is the `kind`).
    let mut c = forcing(1, ExoticInterventionKind::AddSource, 10, 100);
    c.amount = f32::NAN;
    assert!(c.validate(6000).is_err());
    let mut c = forcing(1, ExoticInterventionKind::AddSource, 10, 100);
    c.amount = -1.0;
    assert!(c.validate(6000).is_err());
    let mut c = forcing(1, ExoticInterventionKind::AddSource, 10, 100);
    c.amount = f32::INFINITY;
    assert!(c.validate(6000).is_err());

    // Invalid geometry.
    let mut c = forcing(1, ExoticInterventionKind::AddSource, 10, 100);
    c.region = Region::Radius {
        cx: 0.0,
        cy: 0.0,
        r: -1.0,
    };
    assert!(c.validate(6000).is_err());
    let mut c = forcing(1, ExoticInterventionKind::AddSource, 10, 100);
    c.region = Region::Rect {
        min_x: 9,
        min_y: 0,
        max_x: 1,
        max_y: 1,
    };
    assert!(c.validate(6000).is_err());

    // Overflowing window and a window that can never fire in 1..=run_ticks.
    assert!(
        forcing(1, ExoticInterventionKind::Pulse, u64::MAX - 1, u64::MAX)
            .validate(6000)
            .is_err()
    );
    assert!(forcing(1, ExoticInterventionKind::Pulse, 900_000, 10)
        .validate(6000)
        .is_err());
}

#[test]
fn ae209_m1_queue_is_deterministic_and_rejects_duplicate_ids() {
    // Windows must straddle an ecology firing tick (60) to be valid — see the D3 cadence rule.
    let a = forcing(2, ExoticInterventionKind::AddSource, 30, 50); // [30, 80) ∋ 60
    let b = forcing(1, ExoticInterventionKind::Pulse, 30, 50);
    // Constructed out of order; the queue yields a stable (start_tick, id) order.
    let q = ExoticInterventionQueue::new(vec![a.clone(), b.clone()], 6000).expect("valid queue");
    let ids: Vec<u32> = q.active_at(60).map(|c| c.id).collect();
    assert_eq!(ids, vec![1, 2]);
    assert!(q.active_at(29).next().is_none());
    assert!(q.active_at(80).next().is_none());

    // Duplicate ids are rejected structurally.
    let dup = ExoticInterventionQueue::new(vec![a.clone(), a.clone()], 6000);
    assert!(dup.is_err());

    // An invalid member is rejected by the queue constructor too.
    let mut bad = b.clone();
    bad.amount = f32::NAN;
    assert!(ExoticInterventionQueue::new(vec![bad], 6000).is_err());
}

#[test]
fn ae209_m1_forcing_serde_round_trips() {
    let c = forcing(7, ExoticInterventionKind::RemoveSource, 100, 500);
    let json = serde_json::to_string(&c).expect("serializes");
    let back: ExoticIntervention = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(back, c);
}

// ---- AE-209 (M2): budgeted field forcing ------------------------------------------------

#[test]
fn ae209_m2_add_source_forcing_injects_and_books_mu() {
    let mut law = ExoticEnergyLaw::mana_uniform(50.0);
    law.source_rate = 0.0;
    law.decay_rate = 0.0;
    law.diffusion_rate = 0.0;
    let mut f = build(&law, 8, 8, 1);
    let before_total = f.total();
    let before_sourced = f.cum_sourced;

    let c = forcing(1, ExoticInterventionKind::AddSource, 10, 1);
    let moved = f.apply_forcing(&c, 10);

    assert!(moved > 0.0, "an AddSource forcing must inject MU");
    // The field gained exactly what was booked as sourced.
    assert!((f.total() - before_total - moved).abs() < 1e-6);
    assert!((f.cum_sourced - before_sourced - moved).abs() < 1e-9);
    // The MU ledger still closes.
    let b = f.budget(0.0);
    assert!(b.balance_error().abs() / b.throughput() < 1e-4);
}

// NOTE: the two former tests here — `ae209_m2_remove_source_forcing_withdraws_and_books_as_
// dissipated` and `ae209_m2_removal_cannot_drive_density_negative_and_stays_closed` — encoded the
// ORIGINAL (defective) drain semantics, in which `RemoveSource` debited stored MU and credited
// `cum_dissipated`. That contract was corrected in the audit pass: `RemoveSource` now suppresses
// the base renewable source and never touches stored MU. Those tests are superseded by the `d2_*`
// suite above and were deleted rather than adjusted, so the old behaviour cannot be re-asserted.

#[test]
fn ae209_m2_forcing_is_region_scoped_and_bounded_by_max_density() {
    let mut law = ExoticEnergyLaw::mana_uniform(16.0);
    law.source_rate = 0.0;
    law.decay_rate = 0.0;
    law.diffusion_rate = 0.0;
    law.max_density = 2.0;
    let mut f = build(&law, 4, 4, 1);

    // Region-scoped: only cell (0,0) may change.
    let mut c = forcing(1, ExoticInterventionKind::AddSource, 10, 1);
    c.region = Region::Cell { x: 0, y: 0 };
    c.amount = 1_000.0; // hammer the ceiling
    let before = f.density.clone();
    let moved = f.apply_forcing(&c, 10);

    for (i, (&now, &was)) in f.density.iter().zip(before.iter()).enumerate() {
        if i == 0 {
            assert!(now > was, "the targeted cell must gain");
        } else {
            assert_eq!(
                now, was,
                "cell {i} is outside the region and must not change"
            );
        }
    }
    // Bounded by max_density, and only the ACTUAL increase is booked.
    assert!(f.density[0] <= law.max_density + 1e-6);
    assert!((f.cum_sourced - moved).abs() < 1e-9);
    let b = f.budget(0.0);
    assert!(b.balance_error().abs() / b.throughput() < 1e-4);
}

#[test]
fn ae209_m2_inactive_or_zero_forcing_is_a_noop() {
    let mut law = ExoticEnergyLaw::mana_uniform(20.0);
    law.source_rate = 0.0;
    law.decay_rate = 0.0;
    law.diffusion_rate = 0.0;
    let mut f = build(&law, 4, 4, 1);
    let before = f.density.clone();
    let before_ledger = (f.cum_sourced, f.cum_dissipated);

    let c = forcing(1, ExoticInterventionKind::AddSource, 100, 10);
    // Outside its window.
    assert_eq!(f.apply_forcing(&c, 5), 0.0);
    // Zero magnitude inside the window.
    let mut zero = forcing(2, ExoticInterventionKind::AddSource, 100, 10);
    zero.amount = 0.0;
    assert_eq!(f.apply_forcing(&zero, 100), 0.0);

    assert_eq!(f.density, before);
    assert_eq!((f.cum_sourced, f.cum_dissipated), before_ledger);
}

#[test]
fn ae209_m2_pulse_curve_shapes_the_injection_over_its_window() {
    let mut law = ExoticEnergyLaw::mana_uniform(10.0);
    law.source_rate = 0.0;
    law.decay_rate = 0.0;
    law.diffusion_rate = 0.0;
    law.max_density = 1e6; // never clamp, so we measure the curve itself
    let mut f = build(&law, 4, 4, 1);

    let mut c = forcing(1, ExoticInterventionKind::Pulse, 10, 5);
    c.curve = Curve::Triangle;
    c.amount = 4.0;
    // Triangle peaks mid-window and is ~0 at the edges.
    let at_start = c.amount_at(10);
    let at_mid = c.amount_at(12);
    assert!(at_mid > at_start, "a triangle pulse peaks in the middle");

    let injected_start = f.apply_forcing(&c, 10);
    let injected_mid = f.apply_forcing(&c, 12);
    assert!(injected_mid > injected_start);
    let b = f.budget(0.0);
    assert!(b.balance_error().abs() / b.throughput() < 1e-4);
}

// ---- AUDIT D3: cadence + spatial validation ----------------------------------------------

#[test]
fn d3_forcing_window_must_contain_an_ecology_firing_tick() {
    use crate::core::sim_clock::ECOLOGY_PERIOD;
    assert_eq!(ECOLOGY_PERIOD, 60, "this test encodes the ecology cadence");

    // A one-tick pulse at a NON-ecology tick can never apply — exotic dynamics only fire on the
    // ecology band — so it must be rejected rather than silently ignored.
    let c = forcing(1, ExoticInterventionKind::Pulse, 61, 1);
    assert!(
        c.validate(6000).is_err(),
        "a 1-tick pulse off the ecology band must be rejected"
    );

    // Exactly on an ecology tick is accepted.
    assert!(forcing(1, ExoticInterventionKind::Pulse, 60, 1)
        .validate(6000)
        .is_ok());

    // A window that straddles an ecology tick is accepted...
    assert!(forcing(1, ExoticInterventionKind::AddSource, 59, 2)
        .validate(6000)
        .is_ok());
    // ...but a window that sits strictly between two firings is not.
    assert!(forcing(1, ExoticInterventionKind::AddSource, 61, 59)
        .validate(6000)
        .is_err());
    // [61, 121) contains 120 → accepted (boundary exactness).
    assert!(forcing(1, ExoticInterventionKind::AddSource, 61, 60)
        .validate(6000)
        .is_ok());

    // A window whose only ecology tick lies beyond the run's duration is rejected: [5990, 6010)
    // fires at 6000, which a 5999-tick run never reaches.
    assert!(forcing(1, ExoticInterventionKind::AddSource, 5_990, 20)
        .validate(5_999)
        .is_err());
    // The same window IS valid in a 6000-tick run, where 6000 is the last firing.
    assert!(forcing(1, ExoticInterventionKind::AddSource, 5_990, 20)
        .validate(6_000)
        .is_ok());
    assert!(forcing(1, ExoticInterventionKind::AddSource, 6_000, 1)
        .validate(6_000)
        .is_ok());
}

#[test]
fn d3_grid_applicability_is_validated_against_the_field() {
    let law = ExoticEnergyLaw::mana_uniform(10.0);
    let f = build(&law, 16, 16, 1);

    // In-grid Cell / Rect are fine.
    let mut ok = forcing(1, ExoticInterventionKind::AddSource, 60, 1);
    ok.region = Region::Cell { x: 15, y: 15 };
    assert!(f.validate_region_applicable(&ok).is_ok());
    ok.region = Region::Rect {
        min_x: 0,
        min_y: 0,
        max_x: 15,
        max_y: 15,
    };
    assert!(f.validate_region_applicable(&ok).is_ok());

    // Out-of-grid Cell / Rect are rejected structurally. Rect must be fully contained, not merely
    // overlap the field.
    let mut bad = forcing(2, ExoticInterventionKind::AddSource, 60, 1);
    bad.region = Region::Cell { x: 16, y: 0 };
    assert!(f.validate_region_applicable(&bad).is_err());
    bad.region = Region::Rect {
        min_x: 16,
        min_y: 16,
        max_x: 20,
        max_y: 20,
    };
    assert!(f.validate_region_applicable(&bad).is_err());
    bad.region = Region::Rect {
        min_x: 15,
        min_y: 15,
        max_x: 16,
        max_y: 16,
    };
    assert!(
        f.validate_region_applicable(&bad).is_err(),
        "a partially overflowing Rect must not be silently clipped"
    );

    // A Radius centred off-grid is rejected; one centred in-grid is accepted even if the disc
    // crosses an edge (documented: it is clipped to the grid).
    let mut r = forcing(3, ExoticInterventionKind::AddSource, 60, 1);
    r.region = Region::Radius {
        cx: 100.0,
        cy: 100.0,
        r: 2.0,
    };
    assert!(f.validate_region_applicable(&r).is_err());
    r.region = Region::Radius {
        cx: 0.0,
        cy: 0.0,
        r: 5.0,
    };
    assert!(
        f.validate_region_applicable(&r).is_ok(),
        "an in-grid centre whose disc overhangs the edge is clipped, not rejected"
    );

    // Global always applies.
    let g = forcing(4, ExoticInterventionKind::AddSource, 60, 1);
    assert!(f.validate_region_applicable(&g).is_ok());
}

// ---- AUDIT D2: RemoveSource suppresses the SOURCE, it does not drain the field ------------

#[test]
fn d2_remove_source_never_drains_existing_mu_when_there_is_no_source() {
    // With source_rate = 0 there is nothing to suppress, so a RemoveSource forcing must be a
    // complete no-op on stored MU. (The old semantics drained the field — a sink, not a source
    // removal.)
    let mut law = ExoticEnergyLaw::mana_uniform(80.0);
    law.source_rate = 0.0;
    law.decay_rate = 0.0;
    law.diffusion_rate = 0.0;
    let mut f = build(&law, 8, 8, 1);
    let before = f.density.clone();
    let before_total = f.total();

    let c = forcing(1, ExoticInterventionKind::RemoveSource, 60, 1);
    f.begin_tick_suppression();
    let effective = f.add_source_suppression(&c, 60);
    f.step();

    assert_eq!(effective, 0.0, "nothing to suppress when source_rate == 0");
    assert_eq!(f.density, before, "existing MU must NOT be drained");
    assert_eq!(f.total(), before_total);
    assert_eq!(f.cum_dissipated, 0.0, "removal must not credit dissipation");
    assert_eq!(f.cum_source_suppressed, 0.0);
}

#[test]
fn d2_remove_source_only_avoids_source_injection_and_lowers_cum_sourced() {
    // With a live source, the ONLY difference a RemoveSource makes is the avoided injection.
    let mut law = ExoticEnergyLaw::mana_uniform(20.0);
    law.source_rate = 0.4;
    law.decay_rate = 0.0;
    law.diffusion_rate = 0.0;
    law.max_density = 1e6; // never clamp, so the comparison is exact

    // Unsuppressed reference tick.
    let mut plain = build(&law, 4, 4, 1);
    plain.begin_tick_suppression();
    plain.step();

    // Same tick with a full suppression.
    let mut supp = build(&law, 4, 4, 1);
    let mut c = forcing(1, ExoticInterventionKind::RemoveSource, 60, 1);
    c.amount = 0.4; // exactly the per-cell source contribution
    supp.begin_tick_suppression();
    let effective = supp.add_source_suppression(&c, 60);
    supp.step();

    let cells = (4 * 4) as f64;
    assert!(
        (effective - 0.4 * cells).abs() < 1e-4,
        "effective = {effective}"
    );
    // The suppressed field gained nothing this tick; the plain one gained the full source.
    assert!((plain.cum_sourced - 0.4 * cells).abs() < 1e-4);
    assert_eq!(supp.cum_sourced, 0.0, "suppression lowers cum_sourced");
    assert_eq!(supp.cum_dissipated, 0.0, "and never credits dissipation");
    assert!((supp.cum_source_suppressed - 0.4 * cells).abs() < 1e-4);
    // Existing MU is untouched: the suppressed field still holds its initial amount.
    assert!((supp.total() - 20.0).abs() < 1e-3);
}

#[test]
fn d2_suppression_is_capped_at_the_source_contribution() {
    // A removal far larger than the source can only cancel the source — it can never go further
    // and start eating stored MU.
    let mut law = ExoticEnergyLaw::mana_uniform(30.0);
    law.source_rate = 0.1;
    law.decay_rate = 0.0;
    law.diffusion_rate = 0.0;
    let mut f = build(&law, 4, 4, 1);
    let before_total = f.total();

    let mut c = forcing(1, ExoticInterventionKind::RemoveSource, 60, 1);
    c.amount = 1_000_000.0;
    f.begin_tick_suppression();
    let effective = f.add_source_suppression(&c, 60);
    f.step();

    let cells = 16.0;
    assert!(
        (effective - 0.1 * cells).abs() < 1e-4,
        "capped at the source contribution, got {effective}"
    );
    assert_eq!(f.cum_sourced, 0.0);
    assert_eq!(f.cum_dissipated, 0.0);
    assert!(
        (f.total() - before_total).abs() < 1e-4,
        "stored MU is untouched by an over-large suppression"
    );

    // A saturated field has no realizable renewable contribution. Suppression must therefore be
    // zero rather than claiming that MU "would have entered" a cell with no headroom.
    let mut saturated_law = ExoticEnergyLaw::mana_uniform(160.0);
    saturated_law.source_rate = 0.1;
    saturated_law.decay_rate = 0.0;
    saturated_law.diffusion_rate = 0.0;
    let mut saturated = build(&saturated_law, 4, 4, 1);
    saturated.begin_tick_suppression();
    assert_eq!(saturated.add_source_suppression(&c, 60), 0.0);
    saturated.step();
    assert_eq!(saturated.cum_sourced, 0.0);
    assert_eq!(saturated.cum_source_suppressed, 0.0);
}

#[test]
fn d2_overlapping_suppressions_are_deterministic_and_jointly_capped() {
    let mut law = ExoticEnergyLaw::mana_uniform(10.0);
    law.source_rate = 0.3;
    law.decay_rate = 0.0;
    law.diffusion_rate = 0.0;
    let mut f = build(&law, 4, 4, 1);

    let mut a = forcing(1, ExoticInterventionKind::RemoveSource, 60, 1);
    a.amount = 0.2;
    let mut b = forcing(2, ExoticInterventionKind::RemoveSource, 60, 1);
    b.amount = 0.2; // together 0.4 > the 0.3 source

    f.begin_tick_suppression();
    let ea = f.add_source_suppression(&a, 60);
    let eb = f.add_source_suppression(&b, 60);
    f.step();

    let cells = 16.0;
    // Each command is credited only what it actually contributed; the total is capped.
    assert!((ea - 0.2 * cells).abs() < 1e-4, "first gets its full share");
    assert!((eb - 0.1 * cells).abs() < 1e-4, "second only the remainder");
    assert!(
        (ea + eb - 0.3 * cells).abs() < 1e-4,
        "jointly capped at the source"
    );
    assert_eq!(f.cum_sourced, 0.0);
    assert!((f.cum_source_suppressed - 0.3 * cells).abs() < 1e-4);
}

#[test]
fn d2_add_and_pulse_remain_external_injections() {
    // Add/Pulse are unchanged: they inject MU and credit cum_sourced.
    let mut law = ExoticEnergyLaw::mana_uniform(10.0);
    law.source_rate = 0.0;
    law.decay_rate = 0.0;
    law.diffusion_rate = 0.0;
    let mut f = build(&law, 4, 4, 1);
    let before = f.total();
    let c = forcing(1, ExoticInterventionKind::AddSource, 60, 1);
    let moved = f.apply_forcing(&c, 60);
    assert!(moved > 0.0);
    assert!((f.total() - before - moved).abs() < 1e-6);
    assert!((f.cum_sourced - moved).abs() < 1e-9);
    assert_eq!(f.cum_dissipated, 0.0);
}

#[test]
fn d2_apply_forcing_rejects_remove_source() {
    // RemoveSource is no longer an `apply_forcing` movement; routing it there must be a no-op so
    // the drain semantics cannot be reintroduced by accident.
    let mut law = ExoticEnergyLaw::mana_uniform(10.0);
    law.source_rate = 0.0;
    law.decay_rate = 0.0;
    law.diffusion_rate = 0.0;
    let mut f = build(&law, 4, 4, 1);
    let before = f.density.clone();
    let c = forcing(1, ExoticInterventionKind::RemoveSource, 60, 1);
    assert_eq!(f.apply_forcing(&c, 60), 0.0);
    assert_eq!(f.density, before);
}

// ---- DEFECT P3: transaction helpers reject non-finite / negative inputs ------------------

#[test]
fn p3_uptake_rejects_non_finite_and_negative_inputs_without_mutating() {
    let mut law = ExoticEnergyLaw::mana_uniform(100.0);
    law.source_rate = 0.0;
    law.decay_rate = 0.0;
    law.diffusion_rate = 0.0;
    let mut f = build(&law, 4, 4, 1);
    let before_field = f.density.clone();
    let before_total = f.total();

    // A NaN request must NOT transfer anything. (`f64::min` returns the NON-NaN operand, so a
    // naive `requested <= 0.0` guard lets NaN through and moves real MU.)
    let mut storage = 0.0f64;
    assert_eq!(f.uptake(0, f64::NAN, &mut storage, 1000.0), 0.0);
    assert_eq!(storage, 0.0, "NaN request must not credit storage");
    assert_eq!(
        f.density, before_field,
        "NaN request must not touch the field"
    );

    // Infinite request.
    assert_eq!(f.uptake(0, f64::INFINITY, &mut storage, 1000.0), 0.0);
    assert_eq!(storage, 0.0);
    assert_eq!(f.density, before_field);

    // Negative request.
    assert_eq!(f.uptake(0, -5.0, &mut storage, 1000.0), 0.0);
    assert_eq!(f.density, before_field);

    // Non-finite / negative capacity.
    for cap in [f64::NAN, f64::INFINITY, -1.0] {
        assert_eq!(f.uptake(0, 1.0, &mut storage, cap), 0.0, "capacity {cap}");
        assert_eq!(f.density, before_field);
        assert_eq!(storage, 0.0);
    }

    // A corrupt CURRENT storage value must not be used to compute a transfer.
    let mut bad_storage = f64::NAN;
    assert_eq!(f.uptake(0, 1.0, &mut bad_storage, 1000.0), 0.0);
    assert!(bad_storage.is_nan(), "corrupt storage is left untouched");
    assert_eq!(f.density, before_field);

    // The field total is completely unchanged by every rejected transaction.
    assert_eq!(f.total(), before_total);

    // A valid request still works.
    let mut ok_storage = 0.0f64;
    assert!(f.uptake(0, 1.0, &mut ok_storage, 1000.0) > 0.0);
}

#[test]
fn p3_spend_storage_rejects_non_finite_and_negative_inputs_without_mutating() {
    // A NaN amount must not drain storage (naive `amount <= 0.0` lets NaN through, and
    // `NaN.min(storage)` yields `storage` — spending the whole balance).
    let mut storage = 10.0f64;
    let mut dissipated = 0.0f64;
    assert_eq!(spend_storage(&mut storage, f64::NAN, &mut dissipated), 0.0);
    assert_eq!(storage, 10.0, "NaN amount must not drain storage");
    assert_eq!(dissipated, 0.0, "NaN amount must not credit the sink");

    // Infinite amount.
    assert_eq!(
        spend_storage(&mut storage, f64::INFINITY, &mut dissipated),
        0.0
    );
    assert_eq!(storage, 10.0);
    assert_eq!(dissipated, 0.0);

    // Negative amount.
    assert_eq!(spend_storage(&mut storage, -1.0, &mut dissipated), 0.0);
    assert_eq!(storage, 10.0);
    assert_eq!(dissipated, 0.0);

    // A corrupt CURRENT storage or dissipated ledger must not be spent from / credited into.
    let mut bad_storage = f64::NAN;
    let mut ok_sink = 0.0f64;
    assert_eq!(spend_storage(&mut bad_storage, 1.0, &mut ok_sink), 0.0);
    assert!(bad_storage.is_nan());
    assert_eq!(ok_sink, 0.0);

    let mut ok_storage = 10.0f64;
    let mut bad_sink = f64::NAN;
    assert_eq!(spend_storage(&mut ok_storage, 1.0, &mut bad_sink), 0.0);
    assert_eq!(
        ok_storage, 10.0,
        "storage must not be debited into a corrupt sink"
    );
    assert!(bad_sink.is_nan());

    // A valid spend still works and conserves.
    let mut s = 10.0f64;
    let mut d = 0.0f64;
    assert_eq!(spend_storage(&mut s, 4.0, &mut d), 4.0);
    assert_eq!(s, 6.0);
    assert_eq!(d, 4.0);
}

#[test]
fn deliberate_leak_is_detected_by_the_budget() {
    let law = ExoticEnergyLaw::mana_uniform(100.0);
    let mut f = build(&law, 4, 4, 1);
    // Corrupt the field WITHOUT booking the loss — a leak the audit must catch.
    f.density[0] -= 1e-2;
    let b = f.budget(0.0);
    assert!(
        b.balance_error().abs() > 1e-3,
        "a 1e-2 unbooked leak must be visible in the budget error"
    );
}
