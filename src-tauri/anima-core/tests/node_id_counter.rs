//! Bộ đếm ID node phải tăng đơn điệu và không bao giờ cấp lại một ID.
//!
//! Toán tử 3 lấy `child_id = *node_id_counter` rồi `*node_id_counter += 1`. Nếu
//! phép tăng đó sai — `-=`, `*=`, hay không tăng — thì ID bị cấp lại, hai node
//! khác nhau mang cùng một ID, và `decode_genotype` sẽ nối cạnh vào nhầm node.
//!
//! Năm mutant sống sót ở đúng dòng đó trước khi có file này, vì mọi test chỉ
//! khẳng định "genotype vẫn hợp lệ" — mà hợp lệ vẫn đúng khi ID trùng, do phép
//! kiểm chỉ duyệt theo cạnh chứ không đếm ID.

use anima_core::genotype::{MorphologyGenotype, MorphologyNode};
use anima_core::mutation::mutate_genotype;
use rand::SeedableRng;

fn genotype_mot_node() -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    g.add_node(MorphologyNode { id: 0, length: 1.0, radius: 0.3, mass: 1.0 });
    g
}

#[test]
fn bo_dem_khong_bao_gio_giam() {
    let mut g = genotype_mot_node();
    let mut counter = 1u32;
    let mut rng = rand::rngs::StdRng::seed_from_u64(0x5EED);

    let mut truoc = counter;
    for lan in 0..300 {
        mutate_genotype(&mut g, &mut counter, 1.0, &mut rng);
        assert!(
            counter >= truoc,
            "lan {lan}: bo dem giam tu {truoc} xuong {counter}"
        );
        truoc = counter;
    }
}

#[test]
fn bo_dem_tang_dung_bang_so_node_them_vao() {
    let mut g = genotype_mot_node();
    let mut counter = 1u32;
    let mut rng = rand::rngs::StdRng::seed_from_u64(0xC0FFEE);

    for lan in 0..300 {
        let node_truoc = g.nodes.len();
        let dem_truoc = counter;
        mutate_genotype(&mut g, &mut counter, 1.0, &mut rng);

        // Toán tử 3 thêm đúng một node và tăng bộ đếm đúng một. Toán tử khác
        // không đụng bộ đếm. Nên chênh lệch bộ đếm phải KHỚP số node thêm vào.
        let node_them = g.nodes.len() as i64 - node_truoc as i64;
        let dem_them = counter as i64 - dem_truoc as i64;
        if node_them > 0 {
            assert_eq!(
                dem_them, node_them,
                "lan {lan}: them {node_them} node nhung bo dem tang {dem_them}"
            );
        }
    }
    assert!(g.nodes.len() > 1, "khong lan nao them duoc node — test khong cham nhanh can kiem");
}

#[test]
fn khong_id_nao_bi_cap_lai() {
    let mut g = genotype_mot_node();
    let mut counter = 1u32;
    let mut rng = rand::rngs::StdRng::seed_from_u64(0xBEEF);

    for lan in 0..300 {
        mutate_genotype(&mut g, &mut counter, 1.0, &mut rng);

        let mut ids: Vec<u32> = g.nodes.iter().map(|n| n.id).collect();
        let so_luong = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), so_luong, "lan {lan}: co ID node bi cap lai");

        // Mọi ID đã cấp phải nhỏ hơn bộ đếm — nếu không, lần cấp tiếp theo sẽ trùng.
        for n in &g.nodes {
            assert!(
                n.id < counter,
                "lan {lan}: node co ID {} >= bo dem {counter}, lan cap toi se trung",
                n.id
            );
        }
    }
}
