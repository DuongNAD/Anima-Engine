# fuzz — CHƯA CHẠY ĐƯỢC

`cargo +nightly fuzz build bin_coords` hỏng:

```
src/lib.rs:352: .run(tauri::generate_context!())
                     ^^^ missing field `referenced_by` in ResolvedCommand
```

`generate_context!` cần toàn bộ pipeline build của Tauri, mà build của cargo-fuzz
không cung cấp. Không phải lỗi phiên bản — đã thử ép `fuzz/Cargo.lock` dùng đúng
pin của crate chính, vẫn hỏng.

Xem F-007 trong `FINDINGS.md`. Gỡ được khi tách logic thuần ra crate `anima-core`
không phụ thuộc Tauri.

Target `bin_coords.rs` giữ nguyên ở đây, chạy được ngay khi blocker được gỡ. Giả
thuyết nó định kiểm đã được kiểm bằng tay và **tìm ra bug thật** — xem F-006.
