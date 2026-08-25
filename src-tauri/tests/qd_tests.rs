//! QD gates — feature `ecological-selection`, branch Q.
//!
//! - **QD-S01** — `trophic_ratio` is defined everywhere (no NaN), clamped to [0, 1].
//! - **QD-S02** — the new niche axes are independent of fitness: movement quantities cannot
//!   move them, feeding quantities cannot move axis one.
//! - **QD-S03** — an archive written under a different descriptor set is REFUSED by
//!   `from_saved`, never silently loaded into wrong niches.

use anima_engine_lib::core::ecology::{ecological_descriptors, trophic_ratio};
use anima_engine_lib::evolution::map_elites::{
    MapElitesArchive, SavedEliteIndividual, SavedMapElitesArchive, DESCRIPTOR_SET_ID,
};

// ---------------------------------------------------------------------------------------
// QD-S01 — the ratio itself
// ---------------------------------------------------------------------------------------

#[test]
fn trophic_ratio_is_defined_everywhere_and_clamped() {
    assert_eq!(trophic_ratio(0.0, 0.0), 0.0, "nothing eaten: defined zero");
    assert_eq!(trophic_ratio(3.0, 0.0), 0.0);
    assert!((trophic_ratio(1.0, 4.0) - 0.25).abs() < 1e-6);
    assert_eq!(trophic_ratio(4.0, 4.0), 1.0);
    // Nonsense cannot leak into a niche coordinate.
    assert_eq!(trophic_ratio(9.0, 4.0), 1.0);
    assert_eq!(trophic_ratio(f32::NAN, 4.0), 0.0);
    assert_eq!(trophic_ratio(-2.0, 4.0), 0.0);
}

// ---------------------------------------------------------------------------------------
// QD-S02 — the axes are independent of fitness
// ---------------------------------------------------------------------------------------

#[test]
fn the_trophic_axis_cannot_see_movement() {
    // P-04 regression guard, stated as arithmetic: two animals that ate identically but moved
    // completely differently MUST occupy the same niche coordinates. Under the old axis
    // (foraging range = locomotion fitness minus tick_count) they could not.
    let walker = ecological_descriptors(5.0, trophic_ratio(0.0, 40.0));
    let sprinter = ecological_descriptors(5.0, trophic_ratio(0.0, 400.0));
    let to_bits = |d: [f32; 2]| (d[0].to_bits(), d[1].to_bits());
    assert_eq!(
        to_bits(walker),
        to_bits(sprinter),
        "QD-S02: ten times the movement must not move a niche coordinate"
    );
}

#[test]
fn the_mass_axis_cannot_see_feeding_and_vice_versa() {
    let heavy_grazer = ecological_descriptors(18.0, trophic_ratio(0.0, 30.0));
    let light_predator = ecological_descriptors(2.0, trophic_ratio(28.0, 30.0));
    assert_ne!(heavy_grazer, light_predator);

    // Feeding history moves ONLY axis two.
    let a = ecological_descriptors(10.0, trophic_ratio(0.0, 50.0));
    let b = ecological_descriptors(10.0, trophic_ratio(45.0, 50.0));
    assert_eq!(a[0].to_bits(), b[0].to_bits(), "axis one ignores diet");
    assert_ne!(a[1].to_bits(), b[1].to_bits());
}

// ---------------------------------------------------------------------------------------
// QD-S03 — descriptor-set identity on saved archives
// ---------------------------------------------------------------------------------------

fn saved_archive(id: &str) -> SavedMapElitesArchive {
    SavedMapElitesArchive {
        descriptor_set_id: id.to_owned(),
        grid_resolution: 0.05,
        elites: vec![SavedEliteIndividual {
            bin_x: 2,
            bin_y: 7,
            genotype: anima_engine_lib::evolution::genotype::MorphologyGenotype::new(),
            fitness: 12.5,
            features: vec![0.25, 0.75],
            lineage_id: "lineage-qd".to_owned(),
            generation: 3,
        }],
    }
}

#[test]
fn archives_from_this_build_round_trip_but_foreign_sets_are_refused() {
    // A save written by this build loads back exactly.
    let round = MapElitesArchive::new(0.05).to_saved();
    assert_eq!(round.descriptor_set_id, DESCRIPTOR_SET_ID);
    assert!(MapElitesArchive::from_saved(round).is_ok());

    // An archive written under the OLD descriptor set (foraging-range axis) must be refused:
    // its bin coordinates mean something this build no longer computes.
    let legacy = saved_archive("body-mass+foraging-range/v1");
    let err = match MapElitesArchive::from_saved(legacy) {
        Err(e) => e,
        Ok(_) => panic!("a foreign descriptor set must be refused, not silently loaded"),
    };
    assert!(
        err.contains("body-mass+foraging-range/v1") && err.contains(DESCRIPTOR_SET_ID),
        "the error must name BOTH sets: {err}"
    );

    // A pre-Q save without any field at all reads back as empty and is refused too.
    let pre_q = saved_archive("");
    assert!(MapElitesArchive::from_saved(pre_q).is_err());
}
