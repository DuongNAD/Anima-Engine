//! Bất biến của `mutate_genotype`.
//!
//! Hàm này chọn ngẫu nhiên một trong bốn toán tử, nên không kiểm được "lần này
//! nó làm gì". Nhưng có những điều phải đúng SAU MỌI lần gọi, bất kể toán tử
//! nào chạy — đó là thứ kiểm được, và cũng là thứ 43 mutant sống sót đang phá.

use anima_core::genotype::{MorphologyEdge, MorphologyGenotype, MorphologyNode};
use anima_core::mutation::{is_valid_genotype, mutate_genotype};
use rand::SeedableRng;

const SEED: u64 = 0x5EED;

/// Biên kẹp khai báo trong toán tử 1. Test này là chỗ ghim chúng lại: đổi biên
/// trong src mà không đổi ở đây thì test đỏ, đúng như mong muốn.
const LENGTH: (f32, f32) = (0.1, 5.0);
const RADIUS: (f32, f32) = (0.05, 1.0);
const MASS: (f32, f32) = (0.05, 10.0);

fn node(id: u32, length: f32, radius: f32, mass: f32) -> MorphologyNode {
    MorphologyNode { id, length, radius, mass }
}

fn kiem_bat_bien(g: &MorphologyGenotype, ngu_canh: &str) {
    for n in &g.nodes {
        assert!(
            n.length >= LENGTH.0 && n.length <= LENGTH.1,
            "{ngu_canh}: node {} co length {} ngoai khoang {:?}", n.id, n.length, LENGTH
        );
        assert!(
            n.radius >= RADIUS.0 && n.radius <= RADIUS.1,
            "{ngu_canh}: node {} co radius {} ngoai khoang {:?}", n.id, n.radius, RADIUS
        );
        assert!(
            n.mass >= MASS.0 && n.mass <= MASS.1,
            "{ngu_canh}: node {} co mass {} ngoai khoang {:?}", n.id, n.mass, MASS
        );
    }
}

#[test]
fn tham_so_node_luon_nam_trong_bien_kep() {
    // Bắt đầu TRONG biên nhưng sát mép. Nhiễu của toán tử 1 là +-0.5 cho length,
    // +-0.1 cho radius, +-0.5 cho mass — đủ đẩy vượt mép nên phép kẹp chắc chắn
    // phải làm việc.
    //
    // KHÔNG bắt đầu ngoài biên: toán tử 1 chỉ kẹp node nó bốc trúng, nên node
    // chưa được bốc vẫn giữ giá trị ngoài biên và "mọi node luôn trong biên"
    // không còn là bất biến. Đó là lỗi của test, không phải của code.
    let mut g = MorphologyGenotype::new();
    g.add_node(node(0, 4.9, 0.98, 9.9));
    g.add_node(node(1, 0.15, 0.06, 0.1));

    let mut counter = 2;
    let mut rng = rand::rngs::StdRng::seed_from_u64(SEED);
    for lan in 0..200 {
        mutate_genotype(&mut g, &mut counter, 1.0, &mut rng);
        kiem_bat_bien(&g, &format!("sau lan {lan}"));
    }
}

#[test]
fn neo_khop_luon_nam_trong_bien_cua_node_cha() {
    // Toán tử 1 kẹp neo khớp của mọi cạnh đi ra theo bán kính và độ dài MỚI của
    // node cha: x,y trong [-radius, radius] và z trong [-length/2, length/2].
    let mut g = MorphologyGenotype::new();
    g.add_node(node(0, 4.0, 0.9, 5.0));
    g.add_node(node(1, 4.0, 0.9, 5.0));
    g.add_edge(MorphologyEdge {
        source_node: 0,
        target_node: 1,
        // Trong biên của node cha lúc đầu (radius 0.9, length 4.0), nhưng sát mép:
        // khi toán tử 1 thu nhỏ node cha thì neo bị kẹp lại.
        joint_anchor: glam::Vec3::new(0.88, -0.88, 1.95),
        joint_axis: glam::Vec3::X,
    });

    let mut counter = 2;
    let mut rng = rand::rngs::StdRng::seed_from_u64(SEED);
    let mut da_kep = false;

    for lan in 0..200 {
        mutate_genotype(&mut g, &mut counter, 1.0, &mut rng);
        for e in &g.edges {
            let cha = g.nodes.iter().find(|n| n.id == e.source_node);
            let Some(cha) = cha else { continue };
            let (r, nua_dai) = (cha.radius, cha.length / 2.0);
            assert!(
                e.joint_anchor.x.abs() <= r + 1e-6 && e.joint_anchor.y.abs() <= r + 1e-6,
                "lan {lan}: neo x/y {:?} vuot ban kinh {r} cua node cha {}",
                e.joint_anchor, cha.id
            );
            assert!(
                e.joint_anchor.z.abs() <= nua_dai + 1e-6,
                "lan {lan}: neo z {} vuot nua do dai {nua_dai} cua node cha {}",
                e.joint_anchor.z, cha.id
            );
            if e.joint_anchor.x.abs() <= r + 1e-6 && e.joint_anchor.x.abs() > 0.0 {
                da_kep = true;
            }
        }
    }
    assert!(da_kep, "khong lan nao neo khop bi kep — test khong cham duoc nhanh can kiem");
}

#[test]
fn genotype_luon_hop_le_va_id_khong_trung() {
    let mut g = MorphologyGenotype::new();
    g.add_node(node(0, 1.0, 0.3, 1.0));

    let mut counter = 1;
    let mut rng = rand::rngs::StdRng::seed_from_u64(SEED);
    for lan in 0..200 {
        mutate_genotype(&mut g, &mut counter, 1.0, &mut rng);
        assert!(is_valid_genotype(&g), "lan {lan}: genotype thanh khong hop le");

        let mut ids: Vec<u32> = g.nodes.iter().map(|n| n.id).collect();
        let truoc = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), truoc, "lan {lan}: co id node trung nhau");

        for e in &g.edges {
            assert!(
                g.nodes.iter().any(|n| n.id == e.source_node),
                "lan {lan}: canh tro toi source_node {} khong ton tai", e.source_node
            );
            assert!(
                g.nodes.iter().any(|n| n.id == e.target_node),
                "lan {lan}: canh tro toi target_node {} khong ton tai", e.target_node
            );
        }
    }
}
