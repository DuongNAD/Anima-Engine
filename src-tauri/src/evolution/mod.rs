// Bon module duoi day da chuyen sang crate `anima-core` (khong phu thuoc Tauri).
// Re-export de moi duong dan cu — `crate::evolution::genotype::...` — van chay
// nguyen ven, khong phai sua mot dong nao o cho goi.
pub use anima_core::{crossover, genotype, map_elites, mutation};

pub mod lineage;
pub mod meta_ai;
