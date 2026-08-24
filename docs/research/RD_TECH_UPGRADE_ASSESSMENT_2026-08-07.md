---
title: Đánh giá công nghệ R&D cho đường dữ liệu, Meta-AI và sinh asset 3D
status: proposed
owner: architecture
last_reviewed: 2026-08-07
review_cycle: quarterly
---

# Đánh giá công nghệ R&D ngày 2026-08-07

Tài liệu này giữ bằng chứng và phân loại ứng viên. Nó không thay đổi hợp đồng runtime. Kế hoạch thử
nghiệm tương ứng nằm ở
[`RD_TECH_UPGRADE_PLAN_2026-08-07.md`](../planning/RD_TECH_UPGRADE_PLAN_2026-08-07.md).

## Kết luận

| Ứng viên | Quyết định nghiên cứu | Lý do |
|---|---|---|
| Packed binary cho Tauri data plane | **Pilot** | Simulation tick, pheromone và world artifact là payload lớn, lặp lại; JSON là chi phí có thật cần đo |
| Shared-memory/Apache Arrow | **Defer** | Engine và ECS đang cùng Rust process; Arrow hợp telemetry dạng bảng hơn frame game; chỉ mở lại nếu binary transport không đạt SLO |
| Structured output kiểu PydanticAI | **Adopt pattern, không adopt framework** | Rust đã có `serde`; thêm Python process sẽ tăng dependency và phá ranh giới native |
| Hunyuan3D-2 | **Pilot offline tool** | Có giá trị tạo creature/prop/mesh, nhưng không thuộc simulation tick và cần ngân sách VRAM riêng |
| Classiq/quantum solver | **Reject cho runtime hiện tại** | Chưa có workload, latency hoặc benchmark chứng minh lợi thế so với CPU/GPU cổ điển |
| IBM real-time quantum decoding | **Watch only** | Là tiến bộ phần cứng lượng tử, không nâng cấp simulation/runtime hiện tại |
| “ZCAO” | **Unverified** | Không có tác giả, DOI hoặc repository công khai khớp tiêu đề trong bản tin |

## Bằng chứng từ code hiện tại

### Đường dữ liệu

- World generation đã chạy trong Web Worker và transfer các `ArrayBuffer` về UI theo zero-copy:
  [`worldGen.worker.ts`](../../src/components/Landscape/utils/worldGen.worker.ts).
- IndexedDB giữ typed arrays dạng binary, nhưng lệnh `save_world_artifact` vẫn đổi khoảng 1,05 MiB
  artifact thành `Array.from(Uint8Array)` trước khi qua Tauri:
  [`worldCache.ts`](../../src/components/Landscape/utils/worldCache.ts).
- Emit thread tái sử dụng buffer Rust, nhưng `simulation-tick`, `pheromone-update` và các event khác
  vẫn đi qua `tauri::Emitter`; ghi chú hiện tại ước lượng pheromone JSON khoảng 150–200 KiB mỗi lần:
  [`emit.rs`](../../src-tauri/src/core/emit.rs).
- Benchmark hiện có đo encode/decode/checksum artifact, nhưng chưa đo bridge serialization, JS parse,
  main-thread cost hoặc dropped frames:
  [`BENCHMARKING.md`](../how-to/BENCHMARKING.md).

Kết luận: ưu tiên không phải “thêm zero-copy vào toàn engine”, mà là đo và thay đúng biên JSON có
payload lớn. Control command nhỏ tiếp tục dùng typed JSON vì dễ audit, version và debug.

### Meta-AI

`MetaAiClient` đã có `EnvironmentalEvent` dạng enum và deterministic fallback, nhưng response Gemini
được phân loại bằng substring như `contains("drought")` hoặc `contains("temperature")`:
[`meta_ai.rs`](../../src-tauri/src/evolution/meta_ai.rs).

Pattern nên nhận:

1. yêu cầu structured response/schema từ provider;
2. deserialize trực tiếp vào enum/struct Rust;
3. validate giới hạn nghiệp vụ;
4. tối đa một repair retry ngoài deterministic mode;
5. fallback xác định, không có side effect khi output sai.

Không đưa PydanticAI/Python vào runtime. Nếu provider không hỗ trợ schema, parser Rust vẫn phải
fail-closed thay vì đoán bằng substring.

### Sinh asset 3D

Hunyuan3D-2 phù hợp làm công cụ ngoại tuyến: prompt/image → mesh → kiểm tra → simplify/LOD → GLB →
artifact registry. Không gọi model từ simulation tick hoặc learner.

Repository upstream công bố khoảng 6 GiB VRAM cho shape và 16 GiB cho shape + texture. Máy 16 GiB
vì vậy cần chạy độc quyền GPU, không đồng thời với LIVA LLM, và phải có cancel/unload/recovery.

## Ràng buộc kiến trúc

- Không chia sẻ raw pointer hoặc KV-cache giữa process.
- Không để frontend ghi trực tiếp vào simulation state qua data plane.
- Binary payload phải có magic, version, length cap, checksum khi lưu, và decoder chịu malformed input.
- Deterministic run không được gọi model/network ngoài manifest.
- Dependency/model/asset mới phải qua ADR, license inventory, provenance và rollback theo
  [chính sách nguồn mở](../governance/OPEN_SOURCE_POLICY.md).

## Nguồn upstream

Kiểm ngày 2026-08-07:

- [Apache Arrow](https://github.com/apache/arrow) — IPC format, off-heap buffers và memory mapping.
- [PydanticAI](https://github.com/pydantic/pydantic-ai) — typed output, model abstraction và observability.
- [Hunyuan3D-2](https://github.com/Tencent-Hunyuan/Hunyuan3D-2) — model sizes, VRAM và API server.
- [IBM Quantum Relay-BP FPGA](https://www.ibm.com/quantum/blog/qdc-2025) — công bố dưới 480 ns nhưng còn công việc scale.
- [Classiq blog](https://www.classiq.io/blog) — Quantum Engineering Agents có thật; chưa có bằng chứng phù hợp workload Anima.

