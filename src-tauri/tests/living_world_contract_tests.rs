//! M0 — test ĐỎ khoá hợp đồng living-world (2026-08-25, nhánh `feat/living-world`).
//!
//! Các test ở đây ĐỎ **vì tính năng chưa tồn tại**, không phải vì bug. Khi từng
//! milestone landed (A1, F1, …), từng test chuyển xanh theo đúng DoD trong
//! `docs/ai/implementation/2026-08-25-feature-living-world-tasks.md`.
//!
//! Quy tắc viết: test phải compile được trên cây mã HIỆN TẠI (không tham chiếu
//! field/API chưa tồn tại) và đỏ vì thiếu hành vi — để cả suite vẫn chạy được
//! trong lúc TDD.

use anima_domain::energy::EnergyLedger;
use anima_engine_lib::core::simulation_state::SegmentState;

// ---------------------------------------------------------------------------
// A1 — SegmentState phải chở kích thước thật (tasks §1, A1-1)
// ---------------------------------------------------------------------------

/// Kill 2 sự thật §0: SegmentState hiện chỉ có vị trí/khớp/năng lượng, không có
/// `length`/`radius` — renderer đang vẽ hình cầu cố định 1.6 (LiveAgents.tsx:44).
#[test]
fn a1_segment_state_json_chua_length_va_radius() {
    let state = SegmentState::default();
    let json = serde_json::to_string(&state).expect("SegmentState phải serialize được");

    assert!(
        json.contains("\"length\""),
        "A1-1: SegmentState payload phải chở `length` — hiện chỉ có: {json}"
    );
    assert!(
        json.contains("\"radius\""),
        "A1-1: SegmentState payload phải chở `radius` — hiện chỉ có: {json}"
    );
}

/// A1-1 (phần giá trị): khi field tồn tại, giá trị phải đi nguyên vòng
/// serialize → deserialize (serde snapshot cập nhật).
#[test]
fn a1_segment_state_length_radius_giu_nguyen_qua_json() {
    let json = serde_json::to_string(&SegmentState::default()).expect("serialize");
    let back: SegmentState = serde_json::from_str(&json).expect("deserialize");
    let round_trip = serde_json::to_string(&back).expect("serialize lại");

    assert_eq!(
        json, round_trip,
        "A1-1: serde snapshot của SegmentState phải ổn định qua vòng round-trip"
    );
    // Đỏ cho tới khi A1-1 thêm hai field: sau đó JSON phải mang cả hai giá trị.
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("json hợp lệ");
    assert!(
        parsed.get("length").is_some_and(|v| v.is_number()),
        "A1-1: `length` phải là số — hiện chưa có trong payload"
    );
    assert!(
        parsed.get("radius").is_some_and(|v| v.is_number()),
        "A1-1: `radius` phải là số — hiện chưa có trong payload"
    );
}

// ---------------------------------------------------------------------------
// F1 — Ngăn `dormant` trong EnergyLedger (tasks §3, F1-1; không rollback được)
// ---------------------------------------------------------------------------

/// Kill sự thật §0 số 8: EnergyLedger (anima-domain/src/energy.rs:88) chỉ có
/// baseline/granted/refused/settled/last_total — không có ngăn ngủ đông. F1-1
/// yêu cầu thêm ngăn `dormant` và đưa vào baseline lock.
#[test]
fn f1_energy_ledger_co_ngan_dormant() {
    let ledger = EnergyLedger::default();
    let debug = format!("{ledger:?}");

    assert!(
        debug.contains("dormant"),
        "F1-1: EnergyLedger phải có ngăn `dormant` — hiện chỉ có: {debug}"
    );
}

/// F1-1: ngăn dormant phải khởi tạo bằng 0 và tham gia closed-EU census
/// (residual vẫn ≤ tolerance sau khi hibernate/wake — FD-S01).
/// Đỏ cho tới khi F1-1 thêm field; assertion thứ hai ghim rằng `lock_baseline`
/// vẫn giữ ngữ nghĩa chỉ-lock-một-lần sau khi thêm ngăn mới.
#[test]
fn f1_dormant_khoi_tao_khong_va_vao_baseline_da_khoa() {
    let mut ledger = EnergyLedger::default();
    ledger.lock_baseline(10_000.0);
    // Lock lần hai phải bị bỏ qua (ngữ nghĩa hiện có, D06) — ngăn mới không
    // được phép phá điều này.
    ledger.lock_baseline(999.0);
    assert_eq!(
        ledger.baseline(),
        Some(10_000.0),
        "F1-1: baseline chỉ được lock một lần, cả sau khi thêm ngăn dormant"
    );
    let debug = format!("{ledger:?}");
    assert!(
        debug.contains("dormant: 0"),
        "F1-1: ngăn dormant khởi tạo phải bằng 0 — hiện chỉ có: {debug}"
    );
}
