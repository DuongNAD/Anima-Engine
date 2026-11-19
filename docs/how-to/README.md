---
title: Hướng dẫn thao tác
status: active
owner: maintainers
last_reviewed: 2026-11-17
review_cycle: per-release
---

# How-to

> **Giao việc cho coding agent:** [AGENT_WORKFLOW.md](AGENT_WORKFLOW.md) — hàng đợi thẻ, quyết định
> của chủ dự án đã điền sẵn, luật cho agent và lệnh xác minh trên macOS. Đo hiệu năng trong app:
> [BENCHMARKING.md](BENCHMARKING.md). Chụp và so sánh bản đồ 3D trước/sau một thay đổi hình ảnh
> (`npm run capture:map`, prototype): [MAP_VISUAL_CHECK.md](MAP_VISUAL_CHECK.md). Buổi dùng thử đầu tiên với
> người ngoài (dựng bản chạy trên Windows 1, danh sách kiểm, cài lên máy khác):
> [OWNER_TRIAL_RUN.md](OWNER_TRIAL_RUN.md).

## Trên macOS và Linux

Các khối lệnh trong trang này và trong [BENCHMARKING.md](BENCHMARKING.md) được viết cho **Windows/PowerShell**,
vì đó là máy đích. Lệnh `npm`, `node` và `cargo` chạy y nguyên trong zsh/bash. Những chỗ khác nhau:

| Windows / PowerShell | macOS / zsh | Đã thử trên macOS? |
|---|---|---|
| `$env:TEN = "1"` rồi chạy lệnh | `TEN=1 lệnh`, hoặc `export TEN=1` | Cú pháp shell chuẩn |
| `py scripts\verify_newick.py` | `python3 scripts/verify_newick.py` (cần `pip install dendropy`) | Chạy 2026-11-17 khi **chưa** cài `dendropy`: script dừng với mã thoát 1 và in cách cài. Chưa chạy với `dendropy` |
| `.\src-tauri\target\release\anima-engine.exe` | `./src-tauri/target/release/anima-engine` | Chưa: app chưa được chạy trên máy macOS này |
| `%APPDATA%\com.anima.engine\` (save, capture) | `~/Library/Application Support/com.anima.engine/` | Chưa: thư mục chưa tồn tại ở đây, vì app chưa chạy lần nào. Đường dẫn suy từ `identifier` trong `tauri.conf.json` |
| `Get-NetTCPConnection -LocalPort 5173 -State Listen` | `lsof -nP -iTCP:5173 -sTCP:LISTEN` | Chạy 2026-11-17: không in gì và mã thoát 1 khi không tiến trình nào nghe cổng đó |
| `Get-Content file.json -Raw \| ConvertFrom-Json` | `jq . file.json`, hoặc `python3 -m json.tool file.json` | Công cụ có sẵn; chưa thử trên một file capture thật |
| `cmd /c "cargo test ... > out.txt 2>&1"` | `cargo test ... > out.txt 2>&1` | Đây là dạng `CLAUDE.md` chỉ định cho macOS/Linux |
| `Win+Shift+S` (chụp màn hình) | `Cmd+Shift+4` | Phím tắt hệ thống |

Hai điều không phụ thuộc hệ điều hành và dễ quên: `NOTICE` và hồ sơ giấy phép phải sinh trên Windows
(`gen_notice.mjs --check` báo cũ trên macOS, và đó là hành vi đúng); và checksum của thế giới khác nhau giữa
các nền tảng, nên chỉ so hai lượt chạy trên cùng một máy.

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
npm run tauri:dev
```

Dùng `tauri:dev` / `tauri:build`, không dùng `tauri dev` trần: `tauri.conf.json` không có trường cho
Cargo feature, nên lệnh trần build `default = []` — không Neo4j, không migration giữa shard, learner
CPU — và không báo lỗi gì. Chạy app đầy đủ rất nặng và từng làm treo máy phát triển; chỉ mở khi cần một
con người nhìn.

## Chạy kiểm tra

```powershell
npm run test:frontend
npm run lint
cargo test --manifest-path src-tauri/Cargo.toml --features desktop
```

`--features desktop` là bắt buộc và là thứ CI chạy: các file test bị gate theo feature `networking` hoặc
`ml-wgpu` không được lên lịch nếu thiếu nó.

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

**Harness report** (M0.4) — chạy từ repo root, **sau** `cargo bench --bench tick_systems`:

```powershell
node scripts/bench_baseline.mjs
```

Đọc cách diễn giải và metadata bắt buộc tại
[`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md). Report gồm hai nguồn: các mục `criterion/*` là số
đo thật đọc từ `src-tauri/target/criterion`; phần còn lại là **proxy theo thiết kế** — một vòng fBm, không
phải engine. Trên một clone mới chưa chạy bench, script **từ chối** ghi đè report đã commit (exit 1); đặt
`ANIMA_BENCH_ALLOW_PROXY_ONLY=1` nếu thật sự muốn một report chỉ có proxy. Không so hai report nếu khác
build mode, workload, seed hoặc cấu hình máy mà không ghi rõ.

## Kiểm tra phả hệ bằng một parser bên ngoài

Gate của OSS-070 là **hai nửa trên cùng một file**; không nửa nào đứng một mình có giá trị.

```powershell
cargo test --test newick_export_tests
```

```powershell
py scripts\verify_newick.py
```

Trên macOS/Linux: `python3 scripts/verify_newick.py`. Thiếu `dendropy` thì script dừng với mã thoát khác 0 và
in cách cài, nên một lần chạy "không lỗi" luôn nghĩa là parser đã thật sự đọc fixture.

Nửa đầu (chạy từ `src-tauri/`) ghim output Newick vào
[`tests/fixtures/newick/lineage_forest.nwk`](../../src-tauri/tests/fixtures/newick/lineage_forest.nwk);
nửa sau bắt **DendroPy** đọc chính file đó. Round-trip thuần Rust chỉ chứng minh serializer nhất
quán với chính nó — thứ đáng hỏi là một parser do người chưa từng thấy repo này viết có công nhận
output là cây hợp lệ không.

`dendropy` là **dev-only** (`pip install dendropy`); không có gì trong sản phẩm phụ thuộc Python.
Thiếu nó thì script thoát kèm hướng dẫn cài và nửa Rust vẫn chạy. Chi tiết ở
[kế hoạch nguồn mở §OSS-070](../planning/OPEN_SOURCE_ADOPTION_PLAN.md).

## Tạo/kiểm tra world artifact fixture

Xem script hiện có:

```powershell
npm run gen:artifact-fixture
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
npm run doctor -- --project <đường-dẫn-tới-repo-Anima-Engine>
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
