//! Khoá hành vi 4 toán tử của `mutate_genotype` (anima-core/src/mutation.rs).
//!
//! Trước file này, quét mutation trên `src/mutation.rs` còn 25 mutant sống:
//! xoá cả ba nhánh toán tử cấu trúc/tham số, xoá dấu `!` trong các điều kiện
//! chặn, đổi `+` thành `*` trong công thức nhiễu, và lật các so sánh
//! `>` / `!=` của toán tử bỏ node. Nghĩa là toàn bộ vòng tiến hoá có thể hỏng
//! im lặng mà suite cũ vẫn xanh.
//!
//! Mọi test đều tất định: RNG gieo seed hằng số `0x5EED` lấy từ `HD_SEED`
//! (.hardening.env), mỗi lần chạy dùng một lead offset cố định. Không dùng
//! thread_rng, không dùng đồng hồ.

use anima_core::genotype::{MorphologyEdge, MorphologyGenotype, MorphologyNode};
use anima_core::mutation::{mutate_genotype, is_valid_genotype, NodeIdExhausted};
use glam::Vec3;
use rand::{rngs::StdRng, SeedableRng};

/// HD_SEED trong .hardening.env
const SEED_BASE: u64 = 0x5EED;
const SO_SEED: usize = 128;

fn node(id: u32, length: f32, radius: f32, mass: f32) -> MorphologyNode {
    MorphologyNode { id, length, radius, mass }
}

fn edge(a: u32, b: u32, anchor_z: f32) -> MorphologyEdge {
    MorphologyEdge {
        source_node: a,
        target_node: b,
        joint_anchor: Vec3::new(0.0, 0.0, anchor_z),
        joint_axis: Vec3::X,
    }
}

/// Chuỗi `n` node nối tiếp 0 -> 1 -> ... -> n-1 với tham số cho trước.
fn chain(n: u32, length: f32, radius: f32, mass: f32) -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    for id in 0..n {
        g.add_node(node(id, length, radius, mass));
    }
    for id in 0..n - 1 {
        g.add_edge(edge(id, id + 1, length / 2.0));
    }
    g
}

/// Chạy `mutate_genotype` với mutation_rate = 1.0 trên SO_SEED seed liên tiếp,
/// trả về (kết quả, genotype sau biến đổi) của từng seed.
fn chay(
    g0: &MorphologyGenotype,
    counter: u32,
) -> Vec<(Result<(), NodeIdExhausted>, MorphologyGenotype)> {
    (0..SO_SEED)
        .map(|i| {
            let mut g = g0.clone();
            let mut c = counter;
            let mut rng = StdRng::seed_from_u64(SEED_BASE + i as u64);
            let r = mutate_genotype(&mut g, &mut c, 1.0, &mut rng);
            (r, g)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Toán tử 2 — đột biến tham số cạnh
// ---------------------------------------------------------------------------

#[test]
fn toan_tu_2_nghieng_truc_khop_va_don_anchor_trong_gioi_han() {
    // Hai node một cạnh, trục ban đầu là X, anchor z = 0.5. Toán tử 2 là tay
    // duy nhất vừa nghiêng trục vừa đẩy anchor mà KHÔNG đổi số node/cạnh.
    let mut g0 = chain(2, 1.0, 0.3, 1.0);
    g0.edges[0].joint_anchor = Vec3::new(0.0, 0.0, 0.5);
    let ket_qua = chay(&g0, 2);

    // Nghiêng thật của trục: thành phần y sau chuẩn hoá nằm trong (0.01, 0.5).
    //  - xoá toán tử 2: trục giữ nguyên X (y = 0) -> không seed nào thoả.
    //  - đổi `+` thành `*`: X * nhiễu = (n, 0, 0) -> chuẩn hoá ra (±1,0,0)
    //    (y = 0) hoặc fallback Y (y = 1, văng khỏi biên 0.5) -> không thoả.
    let co_nghieng = ket_qua.iter().any(|(_, g)| {
        if g.nodes.len() != 2 || g.edges.len() != 1 {
            return false;
        }
        let a = g.edges[0].joint_axis;
        a.y.abs() > 0.01 && a.y.abs() < 0.5
    });
    assert!(co_nghieng, "truc khop khong bao gio duoc nghieng");

    // Anchor: x bắt đầu 0.0 — chỉ `0 + nhiễu` mới dời được nó; `0 * nhiễu` = 0 mãi mãi.
    let co_doi_x = ket_qua.iter().any(|(_, g)| {
        g.nodes.len() == 2 && g.edges.len() == 1 && g.edges[0].joint_anchor.x.abs() > 0.02
    });
    assert!(co_doi_x, "anchor.x khong bao gio duoc don");

    // z bắt đầu 0.5, clamp theo length/2 = 0.5: `+ nhiễu` luôn cho >= 0.4;
    // `* nhiễu` cho |z| <= 0.05.
    let co_doi_z = ket_qua.iter().any(|(_, g)| {
        g.nodes.len() == 2 && g.edges.len() == 1 && g.edges[0].joint_anchor.z > 0.35
    });
    assert!(co_doi_z, "anchor.z khong bao gio duoc don");
}

// ---------------------------------------------------------------------------
// Toán tử 3 — thêm node cấu trúc
// ---------------------------------------------------------------------------

#[test]
fn toan_tu_3_them_node_ke_thua_tham_so_va_anchor_hai_phia() {
    // Chuỗi 3 node đặt hết tham số ở trần: con sinh ra bởi toán tử 3 phải kế
    // thừa tham số cha ± nhiễu (vẫn gần trần), chứ không phải tích nhân bẹp.
    let g0 = chain(3, 5.0, 1.0, 10.0);
    let ket_qua = chay(&g0, 3);

    assert!(
        ket_qua.iter().any(|(_, g)| g.nodes.len() > 3),
        "toan tu 3 khong bao gio them duoc node"
    );

    for (_, g) in &ket_qua {
        assert_eq!(is_valid_genotype(g), true, "genotype sau bien doi phai van hop le");
        for n in &g.nodes {
            assert!(n.length > 4.4 && n.length <= 5.0, "length sai: {}", n.length);
            assert!(n.radius > 0.85 && n.radius <= 1.0, "radius sai: {}", n.radius);
            assert!(n.mass > 9.4 && n.mass <= 10.0, "mass sai: {}", n.mass);
        }
    }

    // z_anchor chọn ngẫu nhiên ±length/2: phải tồn tại anchor ÂM — nếu mất dấu
    // `-` thì mọi anchor đều dương.
    let co_anchor_am = ket_qua
        .iter()
        .any(|(_, g)| g.edges.iter().any(|e| e.joint_anchor.z < 0.0));
    assert!(co_anchor_am, "anchor.z phai co ca hai phia");
}

// ---------------------------------------------------------------------------
// Toán tử 4 — bỏ node lá
// ---------------------------------------------------------------------------

#[test]
fn toan_tu_4_bo_node_la_va_giu_tinh_hop_le() {
    let g0 = chain(5, 1.0, 0.3, 1.0);
    let ket_qua = chay(&g0, 5);

    // Phải tồn tại seed mà số node GIẢM — xoá nhánh 4, hoặc lật `len > 2`
    // thành `== 2` / `< 2` (đều sai với 5 node) thì không bao giờ giảm.
    assert!(
        ket_qua.iter().any(|(_, g)| g.nodes.len() < 5),
        "toan tu 4 khong bao gio bo duoc node la"
    );

    // Bất kể toán tử nào chạy, genotype kết quả phải hợp lệ. Lật `!=` thành
    // `==` trong retain sẽ xoá NHẦM node/edge khiến validity vỡ hoặc revert.
    for (_, g) in &ket_qua {
        assert_eq!(is_valid_genotype(g), true, "genotype hong sau bien doi");
    }
}

// ---------------------------------------------------------------------------
// Các ca biên của điều kiện chặn
// ---------------------------------------------------------------------------

#[test]
fn genotype_rong_khong_loi_va_khong_thay_doi() {
    // Genotype rỗng: cả 4 điều kiện chặn đều phải giữ các toán tử im lặng.
    // Xoá dấu `!` trong bất kỳ điều kiện nào sẽ làm toán tử đó chọn chỉ mục
    // trong tập rỗng -> panic "empty range".
    let mut g = MorphologyGenotype::new();
    let mut c = 0u32;
    let mut rng = StdRng::seed_from_u64(SEED_BASE);
    let r = mutate_genotype(&mut g, &mut c, 1.0, &mut rng);
    assert_eq!(r, Ok(()));
    assert_eq!(g.nodes.len(), 0);
    assert_eq!(g.edges.len(), 0);
}

#[test]
fn chu_trinh_dai_khong_la_thi_toan_tu_4_phai_im_lang() {
    // Vòng 15 node: đủ dài để toán tử 3 bị chặn (len < 15 sai), mọi node đều
    // có cạnh ra nên không có lá — toán tử 4 phải tự nhận biết và KHÔNG làm gì.
    // Xoá dấu `!` ở kiểm tra `leaf_candidates.is_empty()` sẽ chọn chỉ mục trong
    // tập rỗng -> panic.
    let n = 15u32;
    let mut g = MorphologyGenotype::new();
    for id in 0..n {
        g.add_node(node(id, 1.0, 0.3, 1.0));
    }
    for id in 0..n {
        let nxt = (id + 1) % n;
        g.add_edge(edge(id, nxt, 0.5));
    }
    let truoc = g.clone();

    let ket_qua = chay(&g, n);
    for (r, g_moi) in &ket_qua {
        assert_eq!(r, &Ok(()));
        assert_eq!(g_moi.nodes.len(), n as usize, "khong duoc them/bot node");
        assert_eq!(g_moi.edges.len(), n as usize, "khong duoc them/bot canh");
        assert_eq!(g_moi.nodes, truoc.nodes);
    }
}


#[test]
fn toan_tu_1_doi_tham_so_node_van_giu_khoang_gan_gia_tri_goc() {
    // Một node đơn, không cạnh: toán tử 2 (cần cạnh) và toán tử 4 (cần >2 node)
    // không bao giờ tác động; toán tử 3 làm node tăng lên 2. Nên mọi lần chạy
    // mà VẪN còn đúng 1 node thì chắc chắn là tay toán tử 1.
    let g0 = chain(1, 5.0, 1.0, 10.0);
    let ket_qua = chay(&g0, 1);

    for (r, _) in &ket_qua {
        assert_eq!(r, &Ok(()), "mutate_genotype không được báo lỗi trên genotype hợp lệ");
    }

    // Phải tồn tại seed mà tham số THẬT SỰ đổi — nếu không thì toán tử 1 bị
    // xoá/vô hiệu mà không ai hay.
    let co_thay_doi = ket_qua.iter().any(|(_, g)| {
        g.nodes.len() == 1
            && (g.nodes[0].length != 5.0 || g.nodes[0].radius != 1.0 || g.nodes[0].mass != 10.0)
    });
    assert!(co_thay_doi, "toan tu 1 khong bao gio doi tham so node");

    // `tham_số + nhiễu` rồi clamp chỉ lệch tối đa nửa bước nhiễu; nếu ai đó đổi
    // `+` thành `*` thì giá trị bị bẹp xuống vùng clamp dưới (<= 2.5 / 0.1 / 5.0).
    for (_, g) in &ket_qua {
        if g.nodes.len() == 1 {
            let n = &g.nodes[0];
            assert!(
                n.length > 4.4 && n.length <= 5.0,
                "length sai khoang sau bien doi: {}",
                n.length
            );
            assert!(
                n.radius > 0.85 && n.radius <= 1.0,
                "radius sai khoang sau bien doi: {}",
                n.radius
            );
            assert!(
                n.mass > 9.4 && n.mass <= 10.0,
                "mass sai khoang sau bien doi: {}",
                n.mass
            );
        }
    }
}

// =====================================================================
// F-015: khoá lại 17 mutant còn sống ở quét tươi 2026-08-24
// (src/mutation.rs — xem mutants.out/missed.txt sau khi sửa compile).
// Mọi test đều tất định: seed hằng số, không RNG phi tất định, không đồng hồ.
// =====================================================================

const SWEEP: usize = 256;
const SWEEP_BASE: u64 = SEED_BASE + 10_000;

/// Chạy `mutate_genotype` với mutation_rate = 1.0 trên SWEEP seed, trả về
/// genotype sau biến đổi của từng seed.
fn quet(g0: &MorphologyGenotype, counter: u32) -> Vec<MorphologyGenotype> {
    (0..SWEEP)
        .map(|i| {
            let mut g = g0.clone();
            let mut c = counter;
            let mut rng = StdRng::seed_from_u64(SWEEP_BASE + i as u64);
            mutate_genotype(&mut g, &mut c, 1.0, &mut rng)
                .expect("mutate_genotype phải thành công với counter còn dư");
            g
        })
        .collect()
}

/// Chain 0 -> 1 với tham số được chọn sao cho mọi clamp của op1/op2 KHÔNG
/// chạm biên: radius sau nhiễu ∈ [0.4, 0.6] > |anchor.x| = |anchor.y| = 0.1;
/// length sau nhiễu ∈ [3.5, 4.5] nên length/2 ∈ [1.75, 2.25] > |anchor.z| = 1.0.
/// Hai cạnh z đối dấu có chủ đích: mutant "/ -> %" trên MAX bound chỉ bộc lộ
/// với z dương, trên MIN bound chỉ bộc lộ với z âm.
fn chain_2_node_dac_biet() -> MorphologyGenotype {
    let mut g = MorphologyGenotype::new();
    g.add_node(node(0, 4.0, 0.5, 2.0));
    g.add_node(node(1, 4.0, 0.5, 2.0));
    g.add_node(node(2, 4.0, 0.5, 2.0));
    // Hai cạnh z đối dấu có chủ đích: mutant "/ -> %" trên MAX bound chỉ bộc lộ
    // với z dương, trên MIN bound chỉ bộc lộ với z âm. Nhắm vào 2 node lá khác
    // nhau (không dùng cạnh trùng 0->1 — DFS của is_valid_genotype sẽ tưởng
    // cycle và revert mọi mutation).
    for (target, z) in [(1u32, 1.0), (2u32, -1.0)] {
        g.add_edge(MorphologyEdge {
            source_node: 0,
            target_node: target,
            joint_anchor: Vec3::new(0.1, 0.1, z),
            joint_axis: Vec3::X,
        });
    }
    g
}

fn tham_so_bi_doi(g: &MorphologyGenotype, g0: &MorphologyGenotype) -> bool {
    g.nodes.iter().zip(g0.nodes.iter()).any(|(a, b)| {
        a.length != b.length || a.radius != b.radius || a.mass != b.mass
    })
}

/// Kill mutant 109:77, 110:77, 112:59/67/81 (op1 clamp anchor):
/// "delete -" biến clamp(-x, x) thành clamp(x, x) — ép anchor vào đúng biên;
/// "/ -> %" biến length/2 thành length%2. Code đúng phải giữ anchor NGUYÊN
/// giá trị vì clamp không chạm.
#[test]
fn op1_giu_nguyen_anchor_khi_clamp_khong_cham() {
    let g0 = chain_2_node_dac_biet();
    let mut op1_runs = 0usize;
    for g in quet(&g0, 10) {
        if !tham_so_bi_doi(&g, &g0) {
            continue; // op2/op3/op4 — anchor đổi giá trị là hợp lệ
        }
        op1_runs += 1;
        for (i, expected_z) in [1.0, -1.0].into_iter().enumerate() {
            assert_eq!(
                g.edges[i].joint_anchor,
                Vec3::new(0.1, 0.1, expected_z),
                "op1 không được đụng anchor khi clamp không chạm — mutant ép vào biên sẽ đổi giá trị này"
            );
        }
    }
    assert!(op1_runs >= 8, "sweep phải phủ seed chạy op1, thấy {op1_runs}");
}

/// Kill mutant 127:84, 129:50, 129:84, 130:68, 131:36/44/58 (op2 anchor):
/// "delete -" ép anchor vào biên (x=y=radius=0.5, z=length/2=2.0);
/// "+ -> *" biến nhiễu cộng thành nhân (y, z âm hoặc gần 0).
#[test]
fn op2_phat_anchor_trong_dai_khong_cham_clamp() {
    let g0 = chain_2_node_dac_biet();
    let mut op2_runs = 0usize;
    for g in quet(&g0, 10) {
        if tham_so_bi_doi(&g, &g0) || g.nodes.len() != 3 || g.edges.len() != 2 {
            continue; // chỉ op2 giữ cấu trúc 3 node/2 cạnh và không đụng tham số node
        }
        op2_runs += 1;
        // Mỗi cạnh hoặc giữ nguyên (op2 chọn cạnh kia), hoặc nằm trong dải
        // nhiễu quanh giá trị gốc. Mutant ép biên/đổi phép tính sẽ lọt ra ngoài cả hai.
        for (e, expected_z) in g.edges.iter().zip([1.0, -1.0]) {
            let nguyen_ban = e.joint_anchor == Vec3::new(0.1, 0.1, expected_z);
            if nguyen_ban {
                continue;
            }
            let a = e.joint_anchor;
            assert!(
                a.x < 0.3,
                "anchor.x phải nằm trong dải nhiễu (0.1 ± 0.1), thấy {} — mutant ép vào biên radius",
                a.x
            );
            assert!(
                a.y >= 0.0 && a.y < 0.3,
                "anchor.y = 0.1 + noise ∈ [0, 0.2), thấy {} — mutant nhân noise cho y âm",
                a.y
            );
            assert!(
                a.z * expected_z > 0.5 && a.z * expected_z < 1.5,
                "anchor.z phải = {} + noise (cùng dấu, |z| ∈ [0.9, 1.1)), thấy {} — mutant biên/chiếu chia đổi giá trị",
                expected_z,
                a.z
            );
        }
    }
    assert!(op2_runs >= 8, "sweep phải phủ seed chạy op2, thấy {op2_runs}");
}

/// Kill mutant 146:41 ("<" -> "<="): op3 phải bị chặn ở ĐÚNG 15 node.
#[test]
fn op3_khong_them_node_khi_dat_tran_15() {
    let g0 = chain(15, 1.0, 0.25, 1.0);
    for g in quet(&g0, 100) {
        assert!(
            g.nodes.len() <= 15,
            "op3 phải bị chặn khi đã đủ 15 node, thấy {}",
            g.nodes.len()
        );
    }
}

/// Kill mutant 194:41 (">" -> ">="): op4 không được bỏ node khi chỉ còn 2.
#[test]
fn op4_khong_bo_node_khi_chi_con_hai() {
    let g0 = chain(2, 1.0, 0.25, 1.0);
    for g in quet(&g0, 100) {
        assert!(
            g.nodes.len() >= 2,
            "op4 không được chạy với 2 node, còn {}",
            g.nodes.len()
        );
    }
}

/// Kill mutant 166:39, 168:40 ("/" -> "%"): anchor z của node con phải là
/// ±parent.length/2 (4.0/2.0 = 2.0), không phải length%2 (4.0%2.0 = 0.0).
#[test]
fn op3_anchor_z_cua_con_la_nua_chieu_dai_cha() {
    let mut g0 = MorphologyGenotype::new();
    g0.add_node(node(0, 4.0, 0.5, 2.0));
    let mut add_runs = 0usize;
    for g in quet(&g0, 7) {
        if g.nodes.len() == 1 {
            continue; // op1 chạy — không có node mới
        }
        add_runs += 1;
        assert_eq!(g.edges.len(), 1, "op3 phải tạo đúng 1 cạnh mới");
        assert_eq!(
            g.edges[0].joint_anchor.z.abs(),
            2.0,
            "anchor.z của con phải = length/2 = 2.0, thấy {}",
            g.edges[0].joint_anchor.z
        );
    }
    assert!(add_runs >= 8, "sweep phải phủ seed chạy op3, thấy {add_runs}");
}

/// Kill mutant 11:9 (Display::fmt -> Ok(Default::default())): thông điệp lỗi
/// phải con người đọc được, không được biến thành chuỗi rỗng.
#[test]
fn loi_node_id_exhausted_co_thong_dieu_ro_rang() {
    assert_eq!(
        format!("{}", NodeIdExhausted),
        "morphology node identity counter exhausted"
    );
}
