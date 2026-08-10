# Luật cho agent

Layout repo này (sinh tự động, đừng sửa tay):
- ngôn ngữ: `rust`
- crate/package: `src-tauri`
- code nguồn: `src-tauri/src`  ← **CHỈ ĐỌC**
- vùng ghi: `src-tauri/tests`   ← **DUY NHẤT được ghi**

Giao diện lệnh duy nhất là `just`. Chạy `just` để xem danh sách.
KHÔNG tự chế lệnh cargo/pytest khác.

## Cấm tuyệt đối
- Sửa bất kỳ file nào trong `src-tauri/src`. Vi phạm = task fail, không ngoại lệ.
- Sửa `justfile`, `AGENTS.md`, `scripts/`, `.hardening.env`, file config build.
- Thêm `#[ignore]` / `@pytest.mark.skip` / `#[should_panic]` để né lỗi.
- Xoá hoặc nới lỏng assertion đã có.
- Dùng nguồn phi tất định trong test: `thread_rng`, `random`, `SystemTime::now`,
  `Instant::now`, `time.time()`, `datetime.now()`.
- Test chỉ khẳng định "không lỗi" (`assert!(x.is_ok())` hoặc `assert result` đứng một mình).
- Tuyên bố hoàn thành khi chưa thấy chuỗi `QUA CONG` từ `just verify`.

## Bắt buộc
- Mọi thay đổi nằm trong `src-tauri/tests` (ngoại lệ duy nhất: `FINDINGS.md`).
- Mọi RNG trong test dùng seed hằng số lấy từ `HD_SEED` trong `.hardening.env`.
- Kết thúc mọi task bằng `just verify "<đối số nếu có>"`. Exit != 0 = CHƯA XONG.
- Báo cáo phải có `file:line` + lệnh tái hiện. Không có thì coi như không báo cáo.
- Tối đa **3 lần** thử sửa. Lần thứ 4 phải dừng và báo thất bại.
- Horizon: nếu quá **20 tool call** mà chưa qua cổng → dừng, báo "task quá lớn".

## Khi bí
Dừng, ghi rõ: "task quá lớn, cần chia nhỏ" + lý do cụ thể.
KHÔNG được nới luật để hoàn thành. Một task fail trung thực có giá trị hơn
một task pass bằng cách hạ chuẩn — vì task fail chỉ ra chỗ cần chia nhỏ,
còn task pass giả tạo làm hỏng toàn bộ số đo.

## Khi phát hiện bug thật trong code nguồn
Không sửa. Ghi vào `FINDINGS.md` theo đúng mẫu có sẵn, rồi báo cáo.
`FINDINGS.md` là metric duy nhất thật sự quan trọng; mutation score chỉ là proxy.
