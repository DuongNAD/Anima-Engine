//! `is_valid_genotype` phải TỪ CHỐI genotype hỏng.
//!
//! Trước khi có file này, mọi lời gọi `is_valid_genotype` trong toàn bộ test đều
//! khẳng định `true` — không một chỗ nào khẳng định `false`. Nghĩa là hàm có thể
//! luôn trả `true` mà cả suite vẫn xanh, và mutation testing xác nhận đúng thế.
//!
//! Điều đó nguy hiểm thật chứ không chỉ là thiếu điểm: `mutate_genotype` dùng
//! chính hàm này để quyết định giữ hay hoàn tác một đột biến. Hàm luôn `true`
//! nghĩa là genotype hỏng được nhận, rồi `decode_genotype` dựng ra sinh vật lỗi.

use anima_core::genotype::{MorphologyEdge, MorphologyGenotype, MorphologyNode};
use anima_core::mutation::is_valid_genotype;
use glam::Vec3;

fn node(id: u32) -> MorphologyNode {
    MorphologyNode { id, length: 1.0, radius: 0.3, mass: 1.0 }
}

fn edge(a: u32, b: u32) -> MorphologyEdge {
    MorphologyEdge {
        source_node: a,
        target_node: b,
        joint_anchor: Vec3::ZERO,
        joint_axis: Vec3::X,
    }
}

#[test]
fn genotype_rong_thi_khong_hop_le() {
    assert_eq!(is_valid_genotype(&MorphologyGenotype::new()), false);
}

#[test]
fn mot_node_khong_canh_thi_hop_le() {
    // Đối chứng: thiếu ca này thì ba test dưới có thể pass nhờ hàm luôn trả false.
    let mut g = MorphologyGenotype::new();
    g.add_node(node(0));
    assert_eq!(is_valid_genotype(&g), true);
}

#[test]
fn hai_goc_thi_khong_hop_le() {
    // Hai node, không cạnh nào -> cả hai đều là gốc. Hình thái phải có đúng một gốc.
    let mut g = MorphologyGenotype::new();
    g.add_node(node(0));
    g.add_node(node(1));
    assert_eq!(is_valid_genotype(&g), false);
}

#[test]
fn node_khong_toi_duoc_thi_khong_hop_le() {
    // 0 -> 1 la mot cay hop le; node 2 va 3 tao thanh mot thanh phan roi rac.
    // Node 2 khong co canh vao nen no cung la mot goc -> bi bat o buoc dem goc.
    let mut g = MorphologyGenotype::new();
    for i in 0..4 { g.add_node(node(i)); }
    g.add_edge(edge(0, 1));
    g.add_edge(edge(2, 3));
    assert_eq!(is_valid_genotype(&g), false);
}

#[test]
fn chu_trinh_thi_khong_hop_le() {
    // 0 -> 1 -> 2 -> 1. Node 1 co canh vao nen goc van duy nhat la 0, nhung
    // duyet se gap lai node 1 -> phat hien chu trinh.
    let mut g = MorphologyGenotype::new();
    for i in 0..3 { g.add_node(node(i)); }
    g.add_edge(edge(0, 1));
    g.add_edge(edge(1, 2));
    g.add_edge(edge(2, 1));
    assert_eq!(is_valid_genotype(&g), false);
}

#[test]
fn canh_tro_toi_node_khong_ton_tai_thi_khong_hop_le() {
    // 0 -> 99 nhung khong co node 99. Duyet se "tham" 99 nen visited.len() vuot
    // so node -> khong bang nhau -> tu choi.
    let mut g = MorphologyGenotype::new();
    g.add_node(node(0));
    g.add_edge(edge(0, 99));
    assert_eq!(is_valid_genotype(&g), false);
}
