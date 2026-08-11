//! Benchmark cho các vòng lặp nóng của sim.
//!
//! Chọn ba hàm này vì chúng chạy mỗi tick với mọi agent, và cả ba đều là tính
//! toán thuần — không I/O, không RNG, không dịch vụ ngoài. Benchmark dính một
//! trong ba thứ đó thì số đo là nhiễu chứ không phải hiệu năng.
//!
//! `perf-gate` so kết quả với baseline và tự revert nếu không thắng có ý nghĩa
//! thống kê. Đây là module DUY NHẤT agent được sửa src/, nên số đo phải sạch.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use anima_engine_lib::ai::cpg::CpgOscillator;
use anima_engine_lib::evolution::map_elites::MapElitesArchive;

fn cpg_tick(c: &mut Criterion) {
    c.bench_function("cpg_tick_1000", |b| {
        b.iter(|| {
            let mut osc = CpgOscillator::new(black_box(3.0), black_box(1.7));
            for _ in 0..1000 {
                osc.tick(black_box(1.0 / 60.0));
            }
            black_box(osc.output)
        })
    });
}

fn map_elites_binning(c: &mut Criterion) {
    let archive = MapElitesArchive::new(0.5);
    c.bench_function("get_bin_coords_1000", |b| {
        b.iter(|| {
            let mut acc = 0i32;
            for i in 0..1000 {
                let f = [black_box(i as f32 * 0.37), black_box(i as f32 * -0.21)];
                let (x, y) = archive.get_bin_coords(&f);
                acc = acc.wrapping_add(x).wrapping_add(y);
            }
            black_box(acc)
        })
    });
}

criterion_group!(benches, cpg_tick, map_elites_binning);
criterion_main!(benches);
