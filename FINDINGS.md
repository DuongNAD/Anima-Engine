# FINDINGS — bug thật tìm được

Đây là metric duy nhất thật sự quan trọng. Mutation score, coverage, số warning
đều chỉ là proxy. Nếu sau một tuần file này vẫn rỗng → verifier chọn sai mục
tiêu, đổi mục tiêu chứ đừng đổ thêm token.

Quy tắc ghi:
- Chỉ ghi khi có test tái hiện được, hoặc log crash cụ thể.
- `Loại` chỉ được là một trong:
  - `BUG` — hành vi sai thật, đã chứng minh
  - `SMELL` — đáng ngờ, chưa chứng minh
  - `TEST-GAP` — code đúng, test thiếu (**không tính vào mục tiêu bug**)
- Không có `file:line` và lệnh tái hiện thì không được ghi.

---

## Mẫu (copy khối này, đừng sửa mẫu gốc)

### F-000 — <tiêu đề một dòng>
- **Ngày:** ____-__-__
- **Loại:** BUG | SMELL | TEST-GAP
- **Vị trí:** `<path>:<line>`
- **Phát hiện bởi:** mutant | fuzz | miri | determinism | dataflow | tay
- **Triệu chứng:** <1–2 câu, hành vi quan sát được sai như thế nào>
- **Tái hiện:** `just ...`
- **Test khoá lại:** `<test file>::<tên test>`
- **Trạng thái:** OPEN | FIXED (<commit>)

---

<!-- ghi mục mới ngay dưới đây -->
