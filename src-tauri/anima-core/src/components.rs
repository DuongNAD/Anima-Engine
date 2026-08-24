//! Kiểm tra tính hợp lệ của payload morphology — bản port thuần của
//! `validate_morphology_payload` trong `anima-engine` (`src/core/components.rs`).
//!
//! Lịch sử: sau merge 157f85d, `map_elites.rs` còn tham chiếu
//! `crate::core::components::…` của crate CHA — path không tồn tại trong
//! anima-core và cũng không thể gọi được (validator ở crate cha là `pub(crate)`,
//! tham chiếu ngược sẽ đảo dependency "crate thuần"). Nên port logic sang đây
//! thay vì gọi chéo. Xem FINDINGS F-015.
//!
//! Khác biệt có chủ đích so với bản gốc: trả về `Result<(), String>` thay vì
//! enum lỗi riêng (call site chỉ format thông điệp, không match variant).

use crate::mutation::is_valid_genotype;
use crate::genotype::MorphologyGenotype;

/// Giữ đồng bộ với `MAX_MIGRATION_MORPHOLOGY_NODES = 128`
/// (`anima-engine/src/core/components.rs`).
pub const MAX_MORPHOLOGY_NODES: usize = 128;

pub fn validate_morphology_payload(
    genotype: &MorphologyGenotype,
) -> Result<(), String> {
    let nodes = genotype.nodes.len();
    if nodes == 0 {
        return Err("empty morphology".to_string());
    }
    if nodes > MAX_MORPHOLOGY_NODES {
        return Err(format!("too many nodes: {nodes} > {MAX_MORPHOLOGY_NODES}"));
    }
    if genotype.edges.len() >= MAX_MORPHOLOGY_NODES {
        return Err(format!(
            "too many edges: {} >= {MAX_MORPHOLOGY_NODES}",
            genotype.edges.len()
        ));
    }
    if !is_valid_genotype(genotype) {
        return Err("invalid topology".to_string());
    }
    if genotype.nodes.iter().any(|node| {
        !node.length.is_finite()
            || !node.radius.is_finite()
            || !node.mass.is_finite()
            || node.length <= 0.0
            || node.radius <= 0.0
            || node.mass <= 0.0
    }) {
        return Err("invalid body dimension".to_string());
    }
    if genotype.edges.iter().any(|edge| {
        let length_squared = edge.joint_axis.length_squared();
        !edge.joint_anchor.is_finite()
            || !edge.joint_axis.is_finite()
            || !length_squared.is_finite()
            || length_squared <= f32::EPSILON
    }) {
        return Err("invalid joint".to_string());
    }
    Ok(())
}
