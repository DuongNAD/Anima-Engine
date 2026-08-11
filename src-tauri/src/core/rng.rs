//! Nguồn ngẫu nhiên tất định cho toàn bộ sim.
//!
//! Luật: KHÔNG nơi nào trong đường chạy sim được gọi `rand::thread_rng()`.
//! Một sim ALife không tái lập được thì không nghiên cứu được — mọi bug tìm
//! thấy sẽ không tái hiện được, và mọi phép đo sẽ lẫn nhiễu.
//!
//! Sim có hai vùng chạy tách biệt nhau nên cần hai luồng RNG:
//!
//! - **trong ECS** — dùng resource [`SimRng`], truy cập qua `ResMut<SimRng>`
//! - **vòng tiến hoá** — chạy ở thread riêng ngoài ECS (xem
//!   `simulation_lifecycle::start`), nên không với tới resource được; nó giữ
//!   `StdRng` riêng lấy từ [`stream`]
//!
//! Hai luồng cùng bắt nguồn từ MỘT seed gốc, nhưng tách dòng bằng `stream_id`
//! để chúng không bao giờ sinh ra cùng một chuỗi số.

use bevy_ecs::system::Resource;
use rand::rngs::StdRng;
use rand::SeedableRng;

/// Seed mặc định. Trùng với `HD_SEED` trong `.hardening.env`.
pub const DEFAULT_SEED: u64 = 0x5EED;

/// Dòng RNG của vòng tiến hoá (chạy ngoài ECS).
pub const STREAM_EVOLUTION: u64 = 1;

/// Tách một seed gốc thành nhiều dòng độc lập.
///
/// Nhân với hằng số Fibonacci băm 64-bit để hai `stream_id` liền nhau không cho
/// ra hai seed gần nhau — `seed_from_u64` của `StdRng` không đảm bảo điều đó.
pub fn stream(seed: u64, stream_id: u64) -> StdRng {
    StdRng::seed_from_u64(seed ^ stream_id.wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

/// RNG dùng chung của mọi bevy system. Lấy qua `ResMut<SimRng>`.
#[derive(Resource)]
pub struct SimRng(pub StdRng);

impl SimRng {
    pub fn from_seed(seed: u64) -> Self {
        Self(StdRng::seed_from_u64(seed))
    }
}

impl Default for SimRng {
    fn default() -> Self {
        Self::from_seed(DEFAULT_SEED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn cung_seed_cho_cung_chuoi() {
        let a: Vec<u32> = (0..8).map(|_| SimRng::from_seed(7).0.gen()).collect();
        let b: Vec<u32> = (0..8).map(|_| SimRng::from_seed(7).0.gen()).collect();
        assert_eq!(a, b);
    }

    #[test]
    fn hai_dong_khac_nhau() {
        let mut x = stream(DEFAULT_SEED, 0);
        let mut y = stream(DEFAULT_SEED, STREAM_EVOLUTION);
        let a: Vec<u32> = (0..8).map(|_| x.gen()).collect();
        let b: Vec<u32> = (0..8).map(|_| y.gen()).collect();
        assert_ne!(a, b, "hai dong RNG trung nhau -> tien hoa va ECS se dong bo sai");
    }
}
