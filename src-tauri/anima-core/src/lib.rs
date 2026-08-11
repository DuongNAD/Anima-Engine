//! Logic tiến hoá thuần — không Tauri, không wgpu, không bevy_ecs.
//!
//! Tách ra từ `anima-engine` để mở khoá `cargo fuzz` và `cargo miri`, cả hai
//! đều bị chặn bởi `tauri::generate_context!` trong crate chính (F-007).
//!
//! Ranh giới: thứ gì cần `World`, `Query`, hay component của bevy thì KHÔNG
//! thuộc về đây. Cầu nối nằm ở `anima_engine_lib::core::spawn`.

pub mod crossover;
pub mod genotype;
pub mod map_elites;
pub mod mutation;
