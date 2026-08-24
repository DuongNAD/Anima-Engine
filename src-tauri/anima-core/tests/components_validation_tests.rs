//! Khoá hành vi `validate_morphology_payload` (anima-core/src/components.rs).
//!
//! Quét tươi 2026-08-24 còn 15 mutant sống ở file này: biến toàn bộ thân hàm
//! thành `Ok(())`, lật biên `>` ở giới hạn 128 node, gộp các điều kiện `||`
//! thành `&&`, xoá dấu `!` trên kiểm tra topology, và lật `<=` trên
//! `length_squared`. Mỗi test dưới đây vi phạm ĐÚNG MỘT ràng buộc để mutant
//! gộp điều kiện không thể trốn.
//!
//! Tất định: không RNG, không đồng hồ.

use anima_core::components::validate_morphology_payload;
use anima_core::genotype::{MorphologyEdge, MorphologyGenotype, MorphologyNode};
use anima_core::mutation::is_valid_genotype;
use glam::Vec3;

fn node(id: u32, length: f32, radius: f32, mass: f32) -> MorphologyNode {
    MorphologyNode { id, length, radius, mass }
}

fn chain(n: u32) -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    for id in 0..n {
        g.add_node(node(id, 1.0, 0.25, 1.0));
    }
    for id in 0..n.saturating_sub(1) {
        g.add_edge(MorphologyEdge {
            source_node: id,
            target_node: id + 1,
            joint_anchor: Vec3::new(0.0, 0.0, 0.5),
            joint_axis: Vec3::X,
        });
    }
    g
}

/// Kill 23:5 (thân hàm -> Ok(())) và 27:14 (">" -> "=="/">="): biên 128 node.
/// Chain 127 node (126 cạnh) hợp lệ; chain 128 node (127 cạnh) cũng hợp lệ vì
/// giới hạn node là ">" chứ không phải ">="; chain 129 node phải bị loại.
#[test]
fn bien_128_node_chinh_xac() {
    assert!(
        validate_morphology_payload(&chain(127)).is_ok(),
        "127 node phải hợp lệ"
    );
    assert!(
        validate_morphology_payload(&chain(128)).is_ok(),
        "128 node (127 cạnh) phải hợp lệ — giới hạn là '>' chứ không phải '>=' hay '=='"
    );
    assert!(
        validate_morphology_payload(&chain(129)).is_err(),
        "129 node phải bị loại"
    );
}

/// Kill 41-45 ("||" -> "&&"): mỗi vi phạm tham số node phải tự mình bị loại.
#[test]
fn tung_vi_pham_tham_so_node_deu_bi_loai() {
    let cases: [(&str, f32, f32, f32); 6] = [
        ("length NaN", f32::NAN, 0.25, 1.0),
        ("radius NaN", 1.0, f32::NAN, 1.0),
        ("mass NaN", 1.0, 0.25, f32::NAN),
        ("length = 0", 0.0, 0.25, 1.0),
        ("radius âm", 1.0, -0.25, 1.0),
        ("mass = 0", 1.0, 0.25, 0.0),
    ];
    for (name, length, radius, mass) in cases {
        let mut g = MorphologyGenotype::new();
        g.add_node(node(0, length, radius, mass));
        assert!(
            validate_morphology_payload(&g).is_err(),
            "trường hợp {name} phải bị loại"
        );
    }
}

/// Kill 51:9 (delete "!" trên is_valid_genotype): topology 2 root phải bị loại
/// dù mọi tham số đều hữu hạn.
#[test]
fn topology_hong_phai_bi_loai() {
    let mut g = MorphologyGenotype::new();
    g.add_node(node(0, 1.0, 0.25, 1.0));
    g.add_node(node(1, 1.0, 0.25, 1.0));
    assert!(!is_valid_genotype(&g), "2 node rời rạc = 2 root = topology hỏng");
    assert!(
        validate_morphology_payload(&g).is_err(),
        "topology hỏng phải bị loại ngay cả khi tham số đẹp"
    );
}

/// Kill 52:13, 53:13, 54:13 ("||" -> "&&") và 54:31 ("<=" -> ">"): mỗi vi phạm
/// trên cạnh phải tự mình bị loại.
#[test]
fn tung_vi_pham_canh_deu_bi_loai() {
    let build = |anchor: Vec3, axis: Vec3| {
        let mut g = MorphologyGenotype::new();
        g.add_node(node(0, 1.0, 0.25, 1.0));
        g.add_node(node(1, 1.0, 0.25, 1.0));
        g.add_edge(MorphologyEdge {
            source_node: 0,
            target_node: 1,
            joint_anchor: anchor,
            joint_axis: axis,
        });
        g
    };

    let cases: [(&str, Vec3, Vec3); 4] = [
        ("anchor NaN", Vec3::NAN, Vec3::X),
        ("axis NaN", Vec3::new(0.0, 0.0, 0.5), Vec3::NAN),
        ("axis = 0 (length_squared = 0)", Vec3::new(0.0, 0.0, 0.5), Vec3::ZERO),
        (
            "axis tràn (length_squared = inf)",
            Vec3::new(0.0, 0.0, 0.5),
            Vec3::new(f32::MAX, 0.0, 0.0),
        ),
    ];
    for (name, anchor, axis) in cases {
        assert!(
            validate_morphology_payload(&build(anchor, axis)).is_err(),
            "trường hợp cạnh {name} phải bị loại"
        );
    }
}