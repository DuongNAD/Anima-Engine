use anima_core::crossover::crossover_genotypes;
use anima_core::genotype::{MorphologyEdge, MorphologyGenotype, MorphologyNode};
use anima_core::map_elites::{
    EliteIndividual, MapElitesArchive, SavedMapElitesArchive,
};
use anima_core::mutation::mutate_genotype;
use glam::Vec3;
use rand::rngs::StdRng;
use rand::SeedableRng;

/// The evolution operators draw from a caller-supplied stream now, so tests seed their own. A
/// failure here reproduces on re-run instead of depending on what `thread_rng` happened to hand out.
fn test_rng() -> StdRng {
    StdRng::seed_from_u64(0xA11CE)
}

#[test]
fn test_map_elites_binning() {
    let archive = MapElitesArchive::new(0.1);

    // Normal features
    let coords = archive.get_bin_coords(&[0.15, 0.25]);
    assert_eq!(coords, (1, 2));

    // Negative features
    let coords_neg = archive.get_bin_coords(&[-0.15, -0.25]);
    assert_eq!(coords_neg, (-2, -3));

    // Empty features fallback to (0, 0)
    let coords_empty = archive.get_bin_coords(&[]);
    assert_eq!(coords_empty, (0, 0));
}

#[test]
fn test_map_elites_archive_empty() {
    let mut archive = MapElitesArchive::new(0.5);

    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode {
        id: 1,
        length: 1.2,
        radius: 0.3,
        mass: 1.0,
    });

    let individual = EliteIndividual {
        genotype,
        fitness: 5.5,
        features: vec![0.2, 0.4],
        lineage_id: "".to_string(),
        generation: 0,
    };

    // Adding to empty niche should succeed
    let added = archive.add_individual(individual);
    assert!(added);
    assert_eq!(archive.grid.len(), 1);

    let stored = archive.grid.get(&(0, 0)).unwrap();
    assert_eq!(stored.fitness, 5.5);
}

#[test]
fn test_map_elites_archive_replace() {
    let mut archive = MapElitesArchive::new(0.5);

    // Create base individual
    let mut gen_a = MorphologyGenotype::new();
    gen_a.add_node(MorphologyNode {
        id: 1,
        length: 1.2,
        radius: 0.3,
        mass: 1.0,
    });
    let ind_a = EliteIndividual {
        genotype: gen_a,
        fitness: 5.5,
        features: vec![0.2, 0.4],
        lineage_id: "".to_string(),
        generation: 0,
    };

    // Create better individual
    let mut gen_b = MorphologyGenotype::new();
    gen_b.add_node(MorphologyNode {
        id: 1,
        length: 1.5,
        radius: 0.3,
        mass: 1.2,
    });
    let ind_b = EliteIndividual {
        genotype: gen_b,
        fitness: 8.2,
        features: vec![0.2, 0.4],
        lineage_id: "".to_string(),
        generation: 0,
    };

    // Create worse individual
    let mut gen_c = MorphologyGenotype::new();
    gen_c.add_node(MorphologyNode {
        id: 1,
        length: 1.0,
        radius: 0.2,
        mass: 0.8,
    });
    let ind_c = EliteIndividual {
        genotype: gen_c,
        fitness: 2.1,
        features: vec![0.2, 0.4],
        lineage_id: "".to_string(),
        generation: 0,
    };

    // Add A (should succeed)
    assert!(archive.add_individual(ind_a));
    assert_eq!(archive.grid.get(&(0, 0)).unwrap().fitness, 5.5);

    // Add C (worse, should fail to replace)
    assert!(!archive.add_individual(ind_c));
    assert_eq!(archive.grid.get(&(0, 0)).unwrap().fitness, 5.5);

    // Add B (better, should replace)
    assert!(archive.add_individual(ind_b));
    assert_eq!(archive.grid.get(&(0, 0)).unwrap().fitness, 8.2);
}

#[test]
fn full_archive_checkpoint_round_trips_selection_state() {
    let mut archive = MapElitesArchive::new(0.25);
    for (id, fitness, features) in [
        ("lineage-a", 5.5, vec![0.2, 0.4]),
        ("lineage-b", 8.2, vec![1.2, 0.1]),
    ] {
        let mut genotype = MorphologyGenotype::new();
        genotype.add_node(MorphologyNode {
            id: fitness as u32,
            length: fitness,
            radius: 0.3,
            mass: 1.0,
        });
        assert!(archive.add_individual(EliteIndividual {
            genotype,
            fitness,
            features,
            lineage_id: id.into(),
            generation: 7,
        }));
    }

    let json = serde_json::to_string(&archive.to_saved()).expect("archive must serialize");
    let saved: SavedMapElitesArchive =
        serde_json::from_str(&json).expect("archive must deserialize");
    let restored = MapElitesArchive::from_saved(saved).expect("valid archive must restore");

    assert_eq!(restored.grid_resolution.to_bits(), 0.25f32.to_bits());
    assert_eq!(restored.grid.len(), archive.grid.len());
    for (coords, before) in &archive.grid {
        let after = restored.grid.get(coords).expect("saved niche must survive");
        assert_eq!(after.lineage_id, before.lineage_id);
        assert_eq!(after.generation, before.generation);
        assert_eq!(after.fitness.to_bits(), before.fitness.to_bits());
        assert_eq!(after.features, before.features);
        assert_eq!(after.genotype.nodes[0].id, before.genotype.nodes[0].id);
    }
}

#[test]
fn full_archive_checkpoint_rejects_duplicate_niches() {
    let mut archive = MapElitesArchive::new(0.25);
    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode {
        id: 1,
        length: 1.0,
        radius: 0.3,
        mass: 1.0,
    });
    assert!(archive.add_individual(EliteIndividual {
        genotype,
        fitness: 1.0,
        features: vec![0.2, 0.4],
        lineage_id: "lineage-a".into(),
        generation: 1,
    }));
    let mut saved = archive.to_saved();
    saved.elites.push(saved.elites[0].clone());

    assert!(
        MapElitesArchive::from_saved(saved).is_err(),
        "a duplicate niche is corrupt state, not a last-write-wins archive"
    );
}

#[test]
fn test_mutate_genotype() {
    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode {
        id: 0,
        length: 2.0,
        radius: 0.5,
        mass: 1.0,
    });

    // With rate 0.0, nothing changes
    let mut counter = 1;
    let original = genotype.clone();
    let mut rng = test_rng();
    mutate_genotype(&mut genotype, &mut counter, 0.0, &mut rng)
        .expect("test node cursor has headroom");
    assert_eq!(genotype.nodes.len(), original.nodes.len());
    assert_eq!(genotype.nodes[0].length, original.nodes[0].length);

    // With rate 1.0, some mutation happens (either parametric perturb or structural add node)
    let mut mutated = false;
    for _ in 0..100 {
        let mut temp_genotype = original.clone();
        let mut temp_counter = 1;
        mutate_genotype(&mut temp_genotype, &mut temp_counter, 1.0, &mut rng)
            .expect("test node cursor has headroom");
        if temp_genotype.nodes.len() == 2
            || temp_genotype.nodes[0].length != 2.0
            || temp_genotype.nodes[0].radius != 0.5
            || temp_genotype.nodes[0].mass != 1.0
        {
            mutated = true;
            break;
        }
    }
    assert!(mutated);
}

#[test]
fn test_crossover_genotypes() {
    let mut parent_a = MorphologyGenotype::new();
    parent_a.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.2,
        mass: 1.0,
    });
    parent_a.add_node(MorphologyNode {
        id: 2,
        length: 1.0,
        radius: 0.2,
        mass: 1.0,
    });

    let mut parent_b = MorphologyGenotype::new();
    parent_b.add_node(MorphologyNode {
        id: 1,
        length: 2.0,
        radius: 0.3,
        mass: 1.5,
    });
    parent_b.add_node(MorphologyNode {
        id: 3,
        length: 2.0,
        radius: 0.3,
        mass: 1.5,
    });

    // Add edge in parent_a
    parent_a.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 2,
        joint_anchor: Vec3::ZERO,
        joint_axis: Vec3::Y,
    });

    // Run crossover
    let mut counter = 4;
    let mut rng = test_rng();
    let child = crossover_genotypes(&parent_a, &parent_b, &mut counter, &mut rng)
        .expect("test node cursor has headroom");

    // The subtree crossover should result in 2 nodes: the root (id 0) and a grafted node from parent_b with remapped id 4.
    assert_eq!(child.nodes.len(), 2);
    assert!(child.nodes.iter().any(|n| n.id == 0));
    assert!(child.nodes.iter().any(|n| n.id == 4));

    // The child should have 1 edge grafting the new node to the root.
    assert_eq!(child.edges.len(), 1);
    assert_eq!(child.edges[0].source_node, 0);
    assert_eq!(child.edges[0].target_node, 4);
}

#[test]
fn test_map_elites_extremes() {
    // 1. Boundary checks on mutation with max node length (>= 15)
    let mut genotype = MorphologyGenotype::new();
    genotype.add_node(MorphologyNode {
        id: 0,
        length: 1.0,
        radius: 0.2,
        mass: 1.0,
    });
    for i in 1..15 {
        genotype.add_node(MorphologyNode {
            id: i,
            length: 1.0,
            radius: 0.2,
            mass: 1.0,
        });
        genotype.add_edge(MorphologyEdge {
            source_node: i - 1,
            target_node: i,
            joint_anchor: Vec3::ZERO,
            joint_axis: Vec3::Y,
        });
    }

    let mut counter = 15;
    let mut rng = test_rng();
    // Mutate multiple times with mutation_rate = 1.0, verify it never exceeds 15 nodes
    for _ in 0..50 {
        mutate_genotype(&mut genotype, &mut counter, 1.0, &mut rng)
            .expect("test node cursor has headroom");
        assert!(genotype.nodes.len() <= 15);
    }

    // 2. Large/Extreme feature coordinates clamping check (must not panic)
    let archive = MapElitesArchive::new(0.5);
    let coords_large = archive.get_bin_coords(&[1e9, -1e9]);
    assert!(coords_large.0 > 0);
    assert!(coords_large.1 < 0);

    // Extreme values check (Infinity, NaN, etc.)
    let coords_inf = archive.get_bin_coords(&[f32::INFINITY, f32::NEG_INFINITY]);
    // Casting infinity to i32 in Rust returns the minimum or maximum integer value, does not panic
    assert!(coords_inf.0 == i32::MAX || coords_inf.0 == i32::MIN);
    assert!(coords_inf.1 == i32::MAX || coords_inf.1 == i32::MIN);
}

// ===========================================================================
// F-015b (2026-08-24): kill 8 mutant còn sống của map_elites.rs (quét tươi).
// Mọi test đều tất định — seed hằng số, không RNG phi tất định, không đồng hồ.
// ===========================================================================

use anima_core::map_elites::SavedEliteIndividual;

fn saved_elite(lineage: &str, fitness: f32) -> SavedEliteIndividual {
    let mut g = MorphologyGenotype::new();
    g.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.25, mass: 1.0 });
    SavedEliteIndividual {
        bin_x: 0,
        bin_y: 0,
        genotype: g,
        fitness,
        features: vec![0.25, 0.25],
        lineage_id: lineage.to_string(),
        generation: 1,
    }
}

/// Kill 68:47 ("||" -> "&&"): resolution finite NHƯNG <= 0 phải bị từ chối.
#[test]
fn from_saved_tu_choi_resolution_khong_duong() {
    for res in [0.0, -1.0, f32::NAN] {
        let saved = SavedMapElitesArchive {
            grid_resolution: res,
            elites: vec![saved_elite("l1", 1.0)],
        };
        assert!(
            MapElitesArchive::from_saved(saved).is_err(),
            "grid_resolution = {res} phải bị từ chối"
        );
    }
}

/// Kill 75:17 ("||" -> "&&"): fitness NaN và feature NaN phải bị từ chối
/// ĐỘC LẬP nhau — chỉ vi phạm một trong hai vẫn phải lỗi.
#[test]
fn from_saved_tu_choi_gia_tri_khong_finite() {
    let fitness_nan = SavedMapElitesArchive {
        grid_resolution: 1.0,
        elites: vec![saved_elite("f", f32::NAN)],
    };
    assert!(
        MapElitesArchive::from_saved(fitness_nan).is_err(),
        "fitness NaN phải bị từ chối"
    );

    let mut feature_nan = SavedMapElitesArchive {
        grid_resolution: 1.0,
        elites: vec![saved_elite("f", 1.0)],
    };
    feature_nan.elites[0].features = vec![0.25, f32::NAN];
    assert!(
        MapElitesArchive::from_saved(feature_nan).is_err(),
        "feature NaN phải bị từ chối"
    );
}

/// Kill 91:17, 92:17 ("||" -> "&&") và 91:49 (">" -> "=="/">="): ràng buộc
/// lineage id — rỗng-sau-trim, ký tự điều khiển, và biên 1024/1025 ký tự.
#[test]
fn from_saved_kiem_soat_lineage_id() {
    let whitespace_only = SavedMapElitesArchive {
        grid_resolution: 1.0,
        elites: vec![saved_elite("   ", 1.0)],
    };
    assert!(
        MapElitesArchive::from_saved(whitespace_only).is_err(),
        "lineage chỉ toàn khoảng trắng phải bị từ chối"
    );

    let control_char = SavedMapElitesArchive {
        grid_resolution: 1.0,
        elites: vec![saved_elite("bad\u{1}id", 1.0)],
    };
    assert!(
        MapElitesArchive::from_saved(control_char).is_err(),
        "lineage chứa ký tự điều khiển phải bị từ chối"
    );

    let boundary: String = std::iter::repeat('x').take(1024).collect();
    let at_limit = SavedMapElitesArchive {
        grid_resolution: 1.0,
        elites: vec![saved_elite(&boundary, 1.0)],
    };
    assert!(
        MapElitesArchive::from_saved(at_limit).is_ok(),
        "lineage đúng 1024 ký tự phải hợp lệ (giới hạn là '>' chứ không phải '>=')"
    );

    let over_limit: String = std::iter::repeat('x').take(1025).collect();
    let over = SavedMapElitesArchive {
        grid_resolution: 1.0,
        elites: vec![saved_elite(&over_limit, 1.0)],
    };
    assert!(
        MapElitesArchive::from_saved(over).is_err(),
        "lineage 1025 ký tự phải bị từ chối"
    );
}

/// Kill 142:35 (">" -> ">="): fitness BẰNG NHAU không được thay thế elite
/// đang chiếm ô — chỉ fitness CAO HƠN mới thay.
#[test]
fn add_individual_khong_thay_the_khi_fitness_bang_nhau() {
    let make = |lineage: &str| {
        let mut g = MorphologyGenotype::new();
        g.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.25, mass: 1.0 });
        EliteIndividual {
            genotype: g,
            fitness: 5.0,
            features: vec![0.25, 0.25],
            lineage_id: lineage.to_string(),
            generation: 1,
        }
    };

    let mut archive = MapElitesArchive::new(1.0);
    assert!(archive.add_individual(make("first")));
    assert!(
        !archive.add_individual(make("second")),
        "fitness bằng nhau không được chiếm ô của elite hiện có"
    );

    let saved = archive.to_saved();
    assert_eq!(saved.elites.len(), 1);
    assert_eq!(
        saved.elites[0].lineage_id, "first",
        "người chiếm ô phải vẫn là elite đầu tiên"
    );
}

/// Kill 173:50 (">" -> ">=") trong select_parent: replay đúng luồng rút.
/// Grid có a(5.0), b(5.0) ở hai ô khác nhau — fitness BẰNG nhau. Code đúng
/// chỉ thay best khi fitness CAO HƠN, nên với cặp equal-fitness khác lineage
/// (vd rút a rồi b) best phải giữ nguyên là a; mutant ">=" thay cả khi bằng
/// rồi trả về b. Mô phỏng 2 lần rút bằng stream RNG giống hệt để tính expected.
#[test]
fn select_parent_replay_khop_mo_phong_rut() {
    use rand::seq::IteratorRandom;
    let mut archive = MapElitesArchive::new(1.0);
    for (i, (lineage, fitness)) in [("a", 5.0f32), ("b", 5.0), ("c", 1.0)].into_iter().enumerate() {
        let mut g = MorphologyGenotype::new();
        g.add_node(MorphologyNode { id: i as u32, length: 1.0, radius: 0.25, mass: 1.0 });
        archive.add_individual(EliteIndividual {
            genotype: g,
            fitness,
            features: vec![i as f32 + 0.25, 0.25],
            lineage_id: lineage.to_string(),
            generation: 1,
        });
    }

    // Thứ tự key BTreeMap: (0,0) < (1,0) < (2,0) tương ứng a, b, c.
    let order = ["a", "b", "c"];
    let fitness = |l: &str| if l == "c" { 1.0f32 } else { 5.0 };
    let mut discriminating = 0usize;
    for i in 0..256u64 {
        let mut rng_sys = StdRng::seed_from_u64(0xD00D + i);
        let mut rng_ref = StdRng::seed_from_u64(0xD00D + i);
        let d1 = *order.iter().choose(&mut rng_ref).unwrap();
        let d2 = *order.iter().choose(&mut rng_ref).unwrap();
        let expected = if fitness(d2) > fitness(d1) { d2 } else { d1 };
        if fitness(d2) == fitness(d1) && d1 != d2 {
            discriminating += 1; // case này phân biệt ">" với ">="
        }
        let best = archive.select_parent(2.0, &mut rng_sys).unwrap();
        assert_eq!(
            best.lineage_id, expected,
            "seed {i}: rút {d1} rồi {d2} — code đúng ('>') phải trả về '{expected}'"
        );
    }
    assert!(
        discriminating >= 20,
        "phải có đủ case equal-fitness khác lineage để phân biệt '>' với '>=': {discriminating}"
    );
}
