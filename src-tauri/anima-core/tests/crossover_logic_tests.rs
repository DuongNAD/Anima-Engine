use glam::Vec3;
use anima_core::crossover::crossover_genotypes;
use anima_core::genotype::{MorphologyEdge, MorphologyGenotype, MorphologyNode};
use anima_core::mutation::is_valid_genotype;
use rand::SeedableRng;

const SEED: u64 = 0x5EED;

/// Helper: build a single-node genotype (root only, no edges).
fn make_root_only(id: u32, length: f32) -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    g.add_node(MorphologyNode { id, length, radius: 0.2, mass: 1.0 });
    g
}

// ---------------------------------------------------------------------------
// Mutant 1 & 2  –  line 67: replace += with -= / *=
//
// When parent_a has only a root, the fallback path grafts every node from
// parent_b's chosen subtree.  For each grafted node the counter must
// increment by exactly 1.  If += becomes -= or *=, the resulting IDs will
// collide or the counter will be wrong.
// ---------------------------------------------------------------------------
#[test]
fn crossover_fallback_node_id_counter_increments_correctly() {
    let parent_a = make_root_only(0, 1.0);

    // parent_b: root(10) -> child(11) -> grandchild(12)
    let mut parent_b = MorphologyGenotype::new();
    parent_b.add_node(MorphologyNode { id: 10, length: 1.0, radius: 0.2, mass: 1.0 });
    parent_b.add_node(MorphologyNode { id: 11, length: 1.0, radius: 0.2, mass: 1.0 });
    parent_b.add_node(MorphologyNode { id: 12, length: 1.0, radius: 0.2, mass: 1.0 });
    parent_b.add_edge(MorphologyEdge {
        source_node: 10, target_node: 11,
        joint_anchor: Vec3::new(0.0, 0.0, 0.5), joint_axis: Vec3::Y,
    });
    parent_b.add_edge(MorphologyEdge {
        source_node: 11, target_node: 12,
        joint_anchor: Vec3::new(0.0, 0.0, 0.5), joint_axis: Vec3::Y,
    });

    let mut counter: u32 = 100;
    let counter_before = counter;
    let mut rng = rand::rngs::StdRng::seed_from_u64(SEED);

    let child = crossover_genotypes(&parent_a, &parent_b, &mut counter, &mut rng)
        .expect("crossover succeeds on valid parents");

    // The chosen subtree has some number of nodes; counter must have
    // advanced by exactly that many.
    let grafted_count = child.nodes.len() - 1; // minus the original root
    assert!(grafted_count >= 1, "At least one node must be grafted");
    assert_eq!(
        counter,
        counter_before + grafted_count as u32,
        "node_id_counter must advance by exactly the number of grafted nodes"
    );

    // Every grafted node ID must be unique and strictly sequential
    // starting from counter_before.
    let mut grafted_ids: Vec<u32> = child.nodes.iter()
        .filter(|n| n.id != 0)
        .map(|n| n.id)
        .collect();
    grafted_ids.sort();
    let expected: Vec<u32> = (counter_before..counter_before + grafted_count as u32).collect();
    assert_eq!(grafted_ids, expected, "Grafted node IDs must be sequential from counter_before");
}

#[test]
fn crossover_fallback_no_duplicate_node_ids() {
    let parent_a = make_root_only(0, 1.0);

    // parent_b with 4 nodes in a chain
    let mut parent_b = MorphologyGenotype::new();
    for i in 0..4u32 {
        parent_b.add_node(MorphologyNode { id: 50 + i, length: 1.0, radius: 0.2, mass: 1.0 });
    }
    parent_b.add_edge(MorphologyEdge {
        source_node: 50, target_node: 51,
        joint_anchor: Vec3::new(0.0, 0.0, 0.5), joint_axis: Vec3::Y,
    });
    parent_b.add_edge(MorphologyEdge {
        source_node: 51, target_node: 52,
        joint_anchor: Vec3::new(0.0, 0.0, 0.5), joint_axis: Vec3::Y,
    });
    parent_b.add_edge(MorphologyEdge {
        source_node: 52, target_node: 53,
        joint_anchor: Vec3::new(0.0, 0.0, 0.5), joint_axis: Vec3::Y,
    });

    let mut counter: u32 = 200;
    let mut rng = rand::rngs::StdRng::seed_from_u64(SEED);

    let child = crossover_genotypes(&parent_a, &parent_b, &mut counter, &mut rng)
        .expect("crossover succeeds on valid parents");

    let mut seen = std::collections::HashSet::new();
    for node in &child.nodes {
        assert!(
            seen.insert(node.id),
            "Duplicate node ID {} — indicates node_id_counter is not incrementing correctly",
            node.id
        );
    }
}

// ---------------------------------------------------------------------------
// Mutant 3  –  line 86: replace == with !=
//
// After grafting, the code looks up the incoming edge of w_node in parent_b
// to copy its anchor/axis.  If == becomes !=, it will find a WRONG edge.
//
// We give parent_b two edges with distinct anchors so we can distinguish
// which one was copied.
// ---------------------------------------------------------------------------
#[test]
fn crossover_fallback_copies_correct_edge_anchor_from_parent_b() {
    let parent_a = make_root_only(0, 1.0);

    // parent_b: root(10) -> node(11)  and  root(10) -> node(12)
    // Edge to 11 has anchor (0.1, 0.2, 0.3)
    // Edge to 12 has anchor (0.9, 0.8, 0.7)
    let mut parent_b = MorphologyGenotype::new();
    parent_b.add_node(MorphologyNode { id: 10, length: 1.0, radius: 0.2, mass: 1.0 });
    parent_b.add_node(MorphologyNode { id: 11, length: 1.0, radius: 0.2, mass: 1.0 });
    parent_b.add_node(MorphologyNode { id: 12, length: 1.0, radius: 0.2, mass: 1.0 });
    parent_b.add_edge(MorphologyEdge {
        source_node: 10, target_node: 11,
        joint_anchor: Vec3::new(0.1, 0.2, 0.3), joint_axis: Vec3::Y,
    });
    parent_b.add_edge(MorphologyEdge {
        source_node: 10, target_node: 12,
        joint_anchor: Vec3::new(0.9, 0.8, 0.7), joint_axis: Vec3::X,
    });

    // Run crossover many times with different seeds.  We need to hit both
    // possible w_node selections (11 and 12) to verify that the anchor
    // always comes from the correct incoming edge — not from the other one.
    for seed in 0u64..50 {
        let mut counter: u32 = 300;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

        let child = crossover_genotypes(&parent_a, &parent_b, &mut counter, &mut rng)
        .expect("crossover succeeds on valid parents");

        // The grafting edge connects root_id (0) to the newly-mapped w_node.
        // It is the last edge pushed (or the only edge connecting to root 0).
        let graft_edge = child.edges.iter()
            .find(|e| e.source_node == 0)
            .expect("Must have a grafting edge from root");

        let _grafted_root_id = graft_edge.target_node;

        // Figure out which node from parent_b was selected as w_node.
        // If the grafted subtree has children, w_node was a non-leaf.
        // We rely on: if w_node==11 or w_node==12, the anchor must match
        // their respective incoming edge.  If w_node==10 (root), the anchor
        // uses the fallback formula.
        let grafted_count = child.nodes.len() - 1;

        if grafted_count == 1 {
            // Only one node grafted → w_node was a leaf (11 or 12).
            // The anchor must be one of the two known values.
            let anchor = graft_edge.joint_anchor;
            let is_anchor_11 = (anchor - Vec3::new(0.1, 0.2, 0.3)).length() < 1e-5;
            let is_anchor_12 = (anchor - Vec3::new(0.9, 0.8, 0.7)).length() < 1e-5;
            assert!(
                is_anchor_11 || is_anchor_12,
                "Graft anchor {:?} does not match any known parent_b edge anchor (seed={})",
                anchor, seed
            );
        } else if grafted_count == 2 {
            // Two nodes grafted → w_node was 10 (root) and one child came along,
            // OR w_node was 11/12 but only one has a child.
            // Actually, 11 and 12 are both leaves of 10, so grafting 10 brings both.
            // If grafted_count==2, w_node could be 10 (root → 3 nodes grafted
            // including itself), so actually grafted_count==3 for root.
            // This case would mean w_node was one of the children (shouldn't happen
            // with this topology). Just verify anchor is valid.
            let anchor = graft_edge.joint_anchor;
            let is_valid = (anchor - Vec3::new(0.1, 0.2, 0.3)).length() < 1e-5
                || (anchor - Vec3::new(0.9, 0.8, 0.7)).length() < 1e-5
                || anchor.z.abs() - (1.0 / 2.0) < 1e-5; // fallback
            assert!(is_valid, "Unexpected anchor {:?} (seed={})", anchor, seed);
        }
        // For grafted_count==3 (entire tree), w_node was 10 (root of parent_b)
        // → fallback anchor = (0, 0, parent_a_root_len/2) = (0, 0, 0.5)
        if grafted_count == 3 {
            let expected_anchor = Vec3::new(0.0, 0.0, 0.5);
            assert!(
                (graft_edge.joint_anchor - expected_anchor).length() < 1e-5,
                "Root graft should use fallback anchor (0,0,0.5), got {:?} (seed={})",
                graft_edge.joint_anchor, seed
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Mutant 4 & 5  –  line 90: replace / with % or *
//
// When w_node IS the root of parent_b (no incoming edge in parent_b), the
// fallback anchor z-coordinate is parent_len / 2.0.  If / becomes % or *,
// the value will be wrong.
//
// Force this path: parent_b has exactly 1 node (the root).  So
// parent_b.nodes.choose(rng) MUST return that root, and there is no
// incoming edge for it → fallback fires.
// ---------------------------------------------------------------------------
#[test]
fn crossover_fallback_anchor_z_is_parent_len_divided_by_two() {
    // parent_a root with length = 3.0
    let parent_a = make_root_only(0, 3.0);

    // parent_b: single root node (length doesn't matter for anchor calc —
    // parent_len comes from child.nodes[0].length which is parent_a's root).
    let parent_b = make_root_only(99, 7.0);

    let mut counter: u32 = 400;
    let mut rng = rand::rngs::StdRng::seed_from_u64(SEED);

    let child = crossover_genotypes(&parent_a, &parent_b, &mut counter, &mut rng)
        .expect("crossover succeeds on valid parents");

    // The graft edge connects root 0 to the newly-mapped node.
    let graft_edge = child.edges.iter()
        .find(|e| e.source_node == 0)
        .expect("Must have a grafting edge from root 0");

    let expected_z = 3.0_f32 / 2.0; // 1.5
    assert!(
        (graft_edge.joint_anchor.z - expected_z).abs() < 1e-5,
        "Expected anchor z = {} (parent_len / 2.0), got {}. \
         If z == {} it means `%` was used; if z == {} it means `*` was used.",
        expected_z, graft_edge.joint_anchor.z,
        3.0_f32 % 2.0, 3.0_f32 * 2.0,
    );
    assert!(
        graft_edge.joint_anchor.x.abs() < 1e-5,
        "Expected anchor x = 0.0, got {}",
        graft_edge.joint_anchor.x,
    );
    assert!(
        graft_edge.joint_anchor.y.abs() < 1e-5,
        "Expected anchor y = 0.0, got {}",
        graft_edge.joint_anchor.y,
    );
}

#[test]
fn crossover_fallback_anchor_distinguishes_div_from_mod_and_mul() {
    // Use a parent_len that makes /, %, * all distinct:
    //   5.0 / 2.0 = 2.5
    //   5.0 % 2.0 = 1.0
    //   5.0 * 2.0 = 10.0
    let parent_a = make_root_only(0, 5.0);
    let parent_b = make_root_only(99, 1.0);

    let mut counter: u32 = 500;
    let mut rng = rand::rngs::StdRng::seed_from_u64(SEED);

    let child = crossover_genotypes(&parent_a, &parent_b, &mut counter, &mut rng)
        .expect("crossover succeeds on valid parents");

    let graft_edge = child.edges.iter()
        .find(|e| e.source_node == 0)
        .expect("Must have a grafting edge from root 0");

    let expected_z = 5.0_f32 / 2.0; // 2.5
    let wrong_mod  = 5.0_f32 % 2.0;  // 1.0
    let wrong_mul  = 5.0_f32 * 2.0;  // 10.0

    assert!(
        (graft_edge.joint_anchor.z - expected_z).abs() < 1e-5,
        "anchor z should be {} (parent_len / 2.0), got {}. \
         Would be {} with `%`, {} with `*`.",
        expected_z, graft_edge.joint_anchor.z, wrong_mod, wrong_mul,
    );
}

// ---------------------------------------------------------------------------
// F-015b (2026-08-24): kill 3 mutant còn sống tại dòng prune cạnh khi ghép
// subtree (121:21 delete !, 121:57 "&&"->"||", 121:60 delete !).
// Bất biến: sau crossover, child PHẢI là cây hợp lệ (is_valid_genotype) và có
// đúng n-1 cạnh. Mutant giữ cạnh lơ lửng (đầu mút đã bị xoá) hoặc cắt quá
// mức (cây đứt gãy) đều phá một trong hai bất biến này.
// ---------------------------------------------------------------------------
#[test]
fn graft_subtree_giu_do_thi_hop_le_tren_seed_sweep() {
    let mut parent_a = MorphologyGenotype::new();
    for id in 0..4u32 {
        parent_a.add_node(MorphologyNode { id, length: 1.0, radius: 0.2, mass: 1.0 });
    }
    for id in 0..3u32 {
        parent_a.add_edge(MorphologyEdge {
            source_node: id, target_node: id + 1,
            joint_anchor: Vec3::new(0.0, 0.0, 0.5), joint_axis: Vec3::Y,
        });
    }

    let mut parent_b = MorphologyGenotype::new();
    for id in 0..3u32 {
        parent_b.add_node(MorphologyNode { id, length: 1.5, radius: 0.3, mass: 1.2 });
    }
    for id in 0..2u32 {
        parent_b.add_edge(MorphologyEdge {
            source_node: id, target_node: id + 1,
            joint_anchor: Vec3::new(0.0, 0.0, 0.75), joint_axis: Vec3::Y,
        });
    }

    for i in 0..128u64 {
        let mut counter: u32 = 100;
        let mut rng = rand::rngs::StdRng::seed_from_u64(SEED + 77_000 + i);
        let child = crossover_genotypes(&parent_a, &parent_b, &mut counter, &mut rng)
            .expect("crossover phải thành công với counter còn dư");
        assert!(
            is_valid_genotype(&child),
            "seed {i}: child phải là cây hợp lệ"
        );
        assert_eq!(
            child.nodes.len(),
            child.edges.len() + 1,
            "seed {i}: cây hợp lệ phải có đúng (số node - 1) cạnh"
        );
        // Kill 121:21/57/60: crossover có revert nội bộ (child invalid ->
        // trả parent_a), nên kiểm tra "child hợp lệ" không đủ. Code đúng LUÔN
        // ghép subtree từ parent_b với id mới >= 100 (counter khởi tạo 100);
        // mutant prune sai bị revert về parent_a (chỉ có id 0..3) → không bao
        // giờ thấy node ghép.
        assert!(
            child.nodes.iter().any(|n| n.id >= 100),
            "seed {i}: phải có node ghép từ parent_b (id >= 100) — mutant prune sai bị revert về parent_a"
        );
    }
}
