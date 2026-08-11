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

## Chọn giá trị đầu vào: tránh phần tử trung hoà

Mutant đổi toán tử chỉ lộ ra khi giá trị đầu vào **phân biệt được** hai toán tử.
Chọn nhầm là test pass mà mutant vẫn sống — và nó sống **im lặng**, vì cổng chỉ
kiểm mutant được giao chứ không kiểm các mutant anh em cùng dòng.

| Toán tử bị thay | Giá trị KHÔNG được dùng | Vì sao |
|---|---|---|
| `*` ↔ `/` | **1.0** | `x * 1 == x / 1` |
| `+` ↔ `-` | **0** | `x + 0 == x - 0` |
| `*` ↔ `+` | **0 hoặc 2** | `2*2 == 2+2`, `0*x == 0+x` khi x=0 |
| `>` ↔ `>=` | giá trị **không bao giờ bằng nhau** | phải có ca hoà mới phân biệt được |
| `&&` ↔ `\|\|` | cả hai vế **cùng** true hoặc cùng false | phải có ca lệch |

Quy tắc: dùng số **không tròn, không trung hoà** — `3.0`, `7`, `0.25`. Và với
toán tử so sánh thì **bắt buộc** có một ca hai vế bằng nhau.

**Giá trị trung hoà còn sinh ra Ở GIỮA phép tính, không chỉ ở tham số.**

Ví dụ thật, đo được: test cho
`phase += 2.0*PI*frequency*delta_time; output = amplitude * sin(phase)`
gọi `tick(0.25)` với `frequency = 1.0`. Kết quả: giết được `+=`→`-=` nhưng để
sống **hai** mutant, vì hai chỗ khác nhau đều rơi vào 1.0:

- `× frequency` với `frequency = 1.0` → `×` và `÷` y hệt
- `× sin(phase)` với `phase = π/2` → **`sin(π/2) = 1.0`**, lại y hệt

Cái thứ hai không nhìn ra từ tham số. Phải tính nhẩm giá trị **trung gian** rồi
hỏi: có chỗ nào thành 0 hay 1 không?

**Và nhớ đi qua mọi nhánh.** Cùng test đó để sống 6 mutant ở nhánh
`if phase > 2π { phase -= 2π }` — `tick(0.25)` cho `phase = π/2`, không bao giờ
chạm tới. Nhánh nào test không đi qua thì mọi mutant trong đó đều sống.

## Test bất biến yếu hơn test nhắm đích — đo được

Cám dỗ thường gặp: viết một test "bao quát" khẳng định bất biến (giá trị luôn
trong khoảng, cấu trúc luôn hợp lệ) rồi hy vọng nó giết cả cụm mutant.

Số đo thật trên Anima-Engine, cùng một hàm:

| Kiểu test | Mutant giết được |
|---|---|
| bất biến (kẹp tham số, biên hình học) | **+2** |
| nhắm đích (bộ đếm tăng đúng bao nhiêu) | **+4** |

Bất biến khẳng định "kết quả nằm trong khoảng rộng" — gần như mọi đột biến đều
giữ được điều đó. Test bất biến vẫn đáng viết cho độ bền, nhưng **đừng dùng nó
làm cách giết mutant**.

Nhắm đích nghĩa là: khẳng định một **quan hệ chính xác**, không phải một khoảng.
"bộ đếm tăng đúng bằng số node thêm vào" giết được; "bộ đếm không giảm" thì không.

## Kiểm cả nhánh TỪ CHỐI, không chỉ nhánh chấp nhận

Nếu hàm trả `bool` mà mọi test đều khẳng định `true`, thì thay cả thân hàm bằng
`true` vẫn qua — và mutation testing sẽ chỉ ra đúng chỗ đó.

Đây là lỗi tìm thấy thật: `is_valid_genotype` được gọi ở 8 chỗ trong test, **cả
8 đều khẳng định `true`**. Hàm kiểm tính hợp lệ có thể hỏng hoàn toàn mà không
ai biết, trong khi chính nó quyết định giữ hay hoàn tác mỗi đột biến.

Với mọi hàm kiểm tra, hỏi: **đã có test nào khẳng định nó trả `false` chưa?**

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
