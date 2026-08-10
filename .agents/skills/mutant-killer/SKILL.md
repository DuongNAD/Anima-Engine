---
name: mutant-killer
description: Writes tests that kill a specific surviving mutant reported by mutation testing. Use when the task mentions a mutant, mutation testing, missed.txt, mutation score, cargo-mutants, mutmut, or a line like "replace fn_name with ()".
---

# Mutant Killer

Đọc `AGENTS.md` ở gốc repo trước. Luật ở đó thắng mọi thứ viết ở đây.

## Đầu vào
Đúng MỘT dòng từ `just list-missed`, dạng:
`<src file>:LINE:COL: replace <fn> -> <T> with <X>`

Nhiều dòng = nhiều task. Không bao giờ gộp.

## Quy trình
1. `just list-missed` nếu chưa có dòng cụ thể.
2. Đọc hàm quanh dòng LINE. **CHỈ ĐỌC.**
3. Viết ra một câu trước khi viết code: *nếu giá trị trả về bị thay thành X,
   hành vi nào quan sát được từ API công khai sẽ sai?*
4. Không trả lời được câu 3 → **dừng**, báo `cần refactor để test được` + lý do.
   Không tự sửa code nguồn.
5. Thêm test vào file test có sẵn nếu chủ đề khớp; chỉ tạo file mới khi không khớp.
   Bắt buộc:
   - seed hằng số (lấy `HD_SEED` trong `.hardening.env`)
   - assert trên **giá trị cụ thể**, không phải `is_ok()`
   - không phụ thuộc timing, số thread, thứ tự HashMap
6. `just verify "<nguyên dòng mutant>"`.
7. Exit != 0 → đọc lỗi, sửa test, thử lại. **Tối đa 3 lần** rồi báo thất bại.

## Cấm
- Chạm code nguồn
- Test chỉ khẳng định "không panic"
- Sửa file config mutation để loại mutant thay vì giết nó
- Báo hoàn thành khi chưa thấy `QUA CONG`

## Báo cáo (bắt buộc, không rút gọn)
```
Mutant:         <nguyên dòng gốc>
Hành vi bị phá: <1 câu>
Test đã thêm:   <file>::<tên test>
Verify:         PASS | FAIL(<lý do cụ thể>)
Nghi ngờ bug:   KHÔNG | CÓ -> F-<số> đã ghi FINDINGS.md
```
