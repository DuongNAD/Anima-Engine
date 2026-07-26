---
title: Hướng dẫn thao tác
status: active
owner: maintainers
last_reviewed: 2026-07-24
review_cycle: per-release
---

# How-to

## Cài và chạy frontend

```powershell
npm install
npm run dev
```

Build production:

```powershell
npm run build
```

Chạy ứng dụng Tauri:

```powershell
npm run tauri dev
```

## Chạy kiểm tra

```powershell
npm run test:frontend
npm run lint
cargo test --manifest-path src-tauri/Cargo.toml
```

Chỉ thay đổi simulation core thì tối thiểu chạy Cargo test; thay codec/artifact/tọa
độ phải chạy cả Rust và frontend để bắt sai khác đa ngôn ngữ.

## Đo baseline

Hai thứ khác nhau, đừng lẫn:

**Số đo thật, theo từng system** (OSS-010) — chạy từ `src-tauri/`:

```powershell
cargo bench --bench tick_systems
```

Cách chạy, cách so mốc, cách gỡ và **bảng baseline đã cam kết** nằm ở
[`BENCHMARKING.md`](BENCHMARKING.md). Đây là bộ trả lời câu "một tick tốn bao nhiêu", vì nó đo từng
system headless — không Tauri, không GPU device.

**Harness report proxy** (M0.4) — chạy từ repo root:

```powershell
node scripts/bench_baseline.mjs
```

Đọc cách diễn giải và metadata bắt buộc tại
[`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md). `timings` của report này **là proxy theo
thiết kế** — một vòng fBm, không phải engine. Không so hai report nếu khác
build mode, workload, seed hoặc cấu hình máy mà không ghi rõ.

## Tạo/kiểm tra world artifact fixture

Xem script hiện có:

```powershell
npx tsx scripts/gen_artifact_fixture.ts
```

Sau khi thay format, phải:

1. tăng version khi thay đổi không tương thích;
2. giữ reader/migration cho version được hỗ trợ;
3. tái tạo fixture có provenance;
4. chạy parity test Rust/TypeScript;
5. cập nhật reference/schema.

## Kiểm tra bản đồ bắt buộc

Theo `AGENTS.md`, mọi công việc liên quan map/terrain/biome/ecosystem/navigation/
collision/water/lighting phải dùng Animal Map Vision theo thứ tự:

1. `discover_map_artifacts`
2. `validate_map_manifest`
3. `prepare_team_review`
4. `inspect_map_views`

Nếu MCP chưa xuất hiện, chạy từ repository của MCP:

```powershell
npm run doctor -- --project E:\Project\Anima-Engine
```

Sau đó reload `/mcp`. Không tuyên bố map hoàn tất nếu thiếu manifest gate, ảnh
before/after chuẩn, navigation reachability hoặc còn mâu thuẫn sinh thái critical/high.

## Thêm dependency nguồn mở

1. Đọc [chính sách nguồn mở](../governance/OPEN_SOURCE_POLICY.md).
2. Ghi upstream/tag/license/integration type.
3. Tạo fixture và baseline trước pilot.
4. Tạo ADR từ [template](../decisions/ADR-0000-template.md).
5. Dùng adapter + feature flag; thêm test và rollback.
6. Cập nhật inventory/NOTICE và tài liệu bị ảnh hưởng.
