---
name: dataflow-audit
description: Produces a machine-checkable data flow model of a codebase as dataflow.json and validates it against the real module dependency graph. Use when the task mentions data flow, dataflow, architecture audit, module dependencies, coupling, or dataflow.json.
---

# Dataflow Audit

"Phân tích luồng dữ liệu" mặc định **không có oracle** — đó đúng là chỗ model
sinh văn xuôi đẹp mà sai. Skill này ép claim thành artifact máy đọc được.

## Quy trình
1. `just dataflow-truth` → sinh `dataflow-truth.dot` (sự thật từ công cụ).
   **Không đọc file này trước khi viết claim.** Đọc trước = gian lận, và sẽ lộ
   ở bước 4 vì invariant sẽ rỗng tuếch.
2. Đọc code nguồn. Viết `dataflow.json` ở gốc repo:
```json
{
  "nodes": ["world", "genome", "gpu_buffer"],
  "edges": [
    {"from": "genome", "to": "world",
     "via": "src/evo/mod.rs:142",
     "invariant": "so luong node giu nguyen qua moi tick"}
  ]
}
```
3. Luật cho từng field:
   - `via` bắt buộc dạng `file.rs:line` — script sẽ từ chối nếu sai định dạng
   - `invariant` phải là mệnh đề **kiểm được**, tối thiểu 3 từ.
     Cấm: "data flows", "passes data", "sends info".
     Đạt: "len giữ nguyên qua tick", "id luôn tăng đơn điệu"
4. `just dataflow-verify`.
   - edge bịa → fail
   - edge có thật mà bỏ sót → fail
   - invariant rỗng tuếch → fail
5. Fail → sửa `dataflow.json`, tối đa 3 lần.

## Sau khi qua cổng
Mỗi `invariant` là một ứng viên test. Chọn 3 cái mạnh nhất, đề xuất tên test
tương ứng trong báo cáo. **Không tự viết** — đó là task riêng cho mutant-killer.

## Báo cáo
```
Node:        <n> | Edge: <m>
Verify:      PASS | FAIL(<edge nào>)
Mâu thuẫn code-vs-doc: <danh sách, hoặc KHÔNG>
Invariant đáng làm test: <3 dòng, mỗi dòng 1 invariant + tên test đề xuất>
```
