//! Golden test: bằng chứng sim tái lập được.
//!
//! `just flaky 20` chỉ chứng minh TEST SUITE ổn định — nó không nói gì về việc
//! hai lần chạy sim từ cùng một seed có cho ra cùng một thế giới hay không.
//! File này mới là bằng chứng đó.
//!
//! Lưu ý 2026-08-24: schedule đã thay đổi sau merge 157f85d (flora bị xoá,
//! xem F-012) nên GOLDEN được ghim lại trong cùng chuỗi sửa — đúng giao thức
//! "hành vi sim đổi có chủ ý" ghi ở dưới.
//!
//! Phạm vi: các system đã chuyển sang `SimRng`. Schedule đầy đủ của sim nằm
//! trong closure của `simulation_lifecycle::start` nên chưa dựng lại được từ
//! test; khi nào tách được nó ra thành hàm dùng chung thì mở rộng test này.

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use anima_engine_lib::core::ecs::{init_world, FoodSpawnSettings, Position};

/// Băm trạng thái quan sát được của thế giới.
///
/// Hai chi tiết bắt buộc, thiếu cái nào là chính golden test thành flaky:
/// - **lượng tử hoá float** trước khi băm: `-0.0` và `0.0` băm ra khác nhau, và
///   sai số cuối chuỗi không có ý nghĩa vật lý
/// - **sắp xếp** trước khi băm: thứ tự duyệt entity phụ thuộc bố cục archetype,
///   không phải thứ nên đưa vào chữ ký trạng thái
fn world_hash(world: &mut World) -> u64 {
    let mut rows: Vec<(i64, i64, i64)> = world
        .query::<&Position>()
        .iter(world)
        .map(|p| {
            let q = |v: f32| (v as f64 * 1.0e6).round() as i64;
            (q(p.0.x), q(p.0.y), q(p.0.z))
        })
        .collect();
    rows.sort_unstable();

    let mut h = DefaultHasher::new();
    rows.len().hash(&mut h);
    rows.hash(&mut h);
    h.finish()
}

fn run_ticks(ticks: usize) -> u64 {
    let mut world = init_world();
    // init_world() KHONG chen FoodSpawnSettings — no duoc chen o
    // simulation_lifecycle. Xem F-002 trong FINDINGS.md: khong co mot duong
    // dung World chuan duy nhat, moi noi tu lap mot tap resource rieng.
    world.insert_resource(FoodSpawnSettings::default());
    // Mot entity van toc hang so de the gioi THUC SU bien doi qua tung tick.
    // Sau merge 157f85d bo flora (F-012), schedule chi con thuc an — bao hoa
    // sau dung 1 tick, khi do hash khong phan biet duoc 1 tick voi 60 tick va
    // test doi chứng o duoi mat het y nghia. update_positions_system tich hop
    // Velocity vao Position moi tick, tat dinh tuyet doi.
    world.spawn((
        Position(glam::Vec3::new(0.0, 0.0, 0.0)),
        anima_engine_lib::core::ecs::Velocity(glam::Vec3::new(0.25, 0.0, -0.1)),
    ));
    let mut schedule = Schedule::default();
    // spawn_flora_system da bi loai khoi src theo merge 157f85d (F-012) —
    // schedule con lai la spawn_food_system + wrap_coordinates_system.
    schedule.add_systems((
        anima_engine_lib::core::ecs::spawn_food_system,
        anima_engine_lib::core::world_systems::update_positions_system,
        anima_engine_lib::core::ecs::wrap_coordinates_system,
    ));
    for _ in 0..ticks {
        schedule.run(&mut world);
    }
    world_hash(&mut world)
}

#[test]
fn determinism_200_ticks() {
    let hashes: Vec<u64> = (0..5).map(|_| run_ticks(200)).collect();
    assert!(
        hashes.windows(2).all(|p| p[0] == p[1]),
        "sim phi tat dinh sau 200 tick: {hashes:?}"
    );
}

#[test]
fn world_hash_bat_duoc_thay_doi_that() {
    // Nếu hash không đổi khi thế giới đổi thì golden test ở trên vô nghĩa —
    // nó sẽ "pass" kể cả khi sim hỏng hoàn toàn.
    assert_ne!(
        run_ticks(1),
        run_ticks(60),
        "world_hash khong phan biet duoc hai trang thai khac nhau"
    );
}

/// Hash ghim cứng — đây mới là thứ bắt được lỗi phi tất định GIỮA CÁC LẦN CHẠY.
///
/// `determinism_200_ticks` so 5 thế giới trong CÙNG một tiến trình, nên nó mù
/// với đúng loại lỗi F-001 (thứ tự duyệt HashMap được gieo lại mỗi lần khởi
/// động tiến trình — trong một tiến trình thì nó ổn định).
///
/// Test này fail có đúng hai nguyên nhân:
/// 1. sim vừa mất tính tất định  -> đi sửa, đừng cập nhật hằng số
/// 2. hành vi sim đổi có chủ ý   -> cập nhật hằng số trong cùng commit với thay đổi đó
#[test]
fn determinism_hash_ghim() {
    const GOLDEN: u64 = 192458580480519260;
    assert_eq!(
        run_ticks(200),
        GOLDEN,
        "world_hash doi -> sim mat tinh tat dinh hoac hanh vi da doi"
    );
}
