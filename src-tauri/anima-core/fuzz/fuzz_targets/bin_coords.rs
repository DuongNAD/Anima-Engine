//! Fuzz `MapElitesArchive::get_bin_coords`.
//!
//! Chọn hàm này vì nó nhận `&[f32]` trực tiếp từ đặc trưng hành vi của agent —
//! tức dữ liệu do sim tính ra, không phải hằng số người viết. Nó chia cho
//! `grid_resolution` rồi `.floor() as i32`. Ba thứ đáng ngờ:
//!   - NaN và ±inf: `as i32` trong Rust là saturating cast, không panic, nhưng
//!     NaN cho 0 — hai đặc trưng NaN khác nhau rơi vào CÙNG một ô lưới
//!   - giá trị cực lớn: bão hoà về i32::MAX/MIN, nhiều đặc trưng khác nhau gộp ô
//!   - grid_resolution rất nhỏ: chia cho gần-không
#![no_main]

use libfuzzer_sys::fuzz_target;
use anima_core::map_elites::MapElitesArchive;

fuzz_target!(|data: &[u8]| {
    if data.len() < 12 {
        return;
    }
    let f = |i: usize| f32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);

    let res = f(0);
    // grid_resolution = 0 hoặc âm là lỗi cấu hình của người gọi, không phải
    // input của sim — bỏ qua để fuzz tập trung vào đường dữ liệu thật.
    if !res.is_finite() || res <= 0.0 {
        return;
    }

    let archive = MapElitesArchive::new(res);
    let features = [f(4), f(8)];
    let _ = archive.get_bin_coords(&features);
});
