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
5. Đếm xem có **bao nhiêu** thứ quan sát được. Toán tử so sánh thường có hai:
   giá trị trả về, VÀ trạng thái bị thay đổi. Khoá một cái thì mutant vẫn sống.
6. Thêm test vào file test có sẵn nếu chủ đề khớp; chỉ tạo file mới khi không khớp.
   Bắt buộc:
   - seed hằng số (lấy `HD_SEED` trong `.hardening.env`)
   - assert trên **giá trị cụ thể**, không phải `is_ok()`
   - không phụ thuộc timing, số thread, thứ tự HashMap
7. Nếu test phụ thuộc RNG: **tự lật toán tử trong code nguồn, chạy test, rồi lật
   lại** — để chắc chắn hai nhánh cho ra kết quả KHÁC nhau. Mất 1 phút, tiết kiệm
   một lượt `just verify` 6 phút. Nhớ lật lại trước khi verify.
8. `just verify "<nguyên dòng mutant>"`.
9. Exit != 0 → đọc lỗi, sửa test, thử lại. **Tối đa 3 lần** rồi báo thất bại.

## Cấm
- Chạm code nguồn (trừ bước 7: lật thử rồi lật lại ngay, không commit)
- Test chỉ khẳng định "không panic"
- Sửa file config mutation để loại mutant thay vì giết nó
- Báo hoàn thành khi chưa thấy `QUA CONG`

## Equivalent mutant — nhận ra sớm, đừng cố giết

Có loại mutant **không thể giết được**: mọi giá trị thay thế đều cho hành vi y hệt
trong môi trường test. Cố viết test cho nó là đốt quota vào việc không tồn tại.

Dấu hiệu, kiểm theo đúng thứ tự này **trước** khi viết dòng test đầu tiên:

1. Giá trị mutant thay vào có **trùng** giá trị thật trong test không?
   Ví dụ `is_online()` luôn `false` khi không có dịch vụ ngoài → thay bằng `false`
   là không đổi gì.
2. Hàm chỉ ghi vào field **private không có getter**, và field đó chỉ được đọc
   trong nhánh không bao giờ chạy?
3. Hàm chỉ có tác dụng phụ ra ngoài (log, mạng, đĩa) mà test không quan sát?

Trúng bất kỳ dấu hiệu nào → **dừng**. Không viết test. Làm hai việc:

```
just skip "<regex khớp mutant>" "<vì sao mọi giá trị thay thế đều cho hành vi y hệt, và điều kiện gỡ ra>"
```

rồi ghi một mục `TEST-GAP` vào `FINDINGS.md`.

**Cẩn thận:** "khó test" **không phải** equivalent. Equivalent nghĩa là *không tồn
tại* test nào phân biệt được với API hiện tại — chứ không phải bạn chưa nghĩ ra.
Không chắc thì cứ viết test; cổng sẽ phán hộ.

## Đọc kết quả verify cho đúng

`TU CHOI: mutant van song` có **hai** nguyên nhân khác hẳn nhau:

- output có `MISSED` → test thật sự chưa phân biệt được hành vi. Sửa test.
- output có `TIMEOUT` → **không phải lỗi của test**. Suite chạy quá lâu.
  Báo lại kèm nguyên dòng TIMEOUT, đừng viết thêm test. Viết thêm test ở đây là
  đốt quota vào việc không tồn tại.

`TU CHOI: bo khung hardening chua duoc commit` cũng không phải lỗi của bạn —
người cài quên commit. Dừng và báo.

## Báo cáo (bắt buộc, không rút gọn)
```
Mutant:         <nguyên dòng gốc>
Hành vi bị phá: <1 câu>
Test đã thêm:   <file>::<tên test>
Verify:         PASS | FAIL(<lý do cụ thể>)
Nghi ngờ bug:   KHÔNG | CÓ -> F-<số> đã ghi FINDINGS.md
```
