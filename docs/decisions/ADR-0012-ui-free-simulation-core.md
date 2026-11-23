---
title: ADR-0012 — Lõi mô phỏng không cần giao diện; giao diện là client chỉ đọc; giao diện cho máy là hạng nhất
status: accepted
owner: architecture
last_reviewed: 2026-11-23
decision_date: 2026-11-23
supersedes: none
superseded_by: none
---

# ADR-0012 — Lõi mô phỏng không cần giao diện; giao diện là client chỉ đọc; giao diện cho máy là hạng nhất

## Bối cảnh

Anima có hai loại người dùng: con người, và các agent (Claude, Gemini) cùng các script kiểm chứng. Từ
tháng 11 năm 2026, phần lớn công việc do agent làm, và mọi kết luận khoa học đều đến từ các lượt chạy
không giao diện:

- VIAB-1 (#215, 1.920 lượt), E3-X (#181) và smoke SEL2-P (#170) chạy qua `experiment_runner`, các
  example `run_*_experiment` và `anima-lab`, không mở app desktop lần nào. Kết quả là JSON, CSV và
  checksum; các bộ kiểm độc lập (`verify_e2_artifacts.mjs`, `verify_e3_artifacts.mjs`) đọc thẳng các file đó.
- Ngày 2026-11-23, toàn bộ việc thẩm định 20 pull request của lead (khoảng 25 lượt review) được làm mà
  không cần giao diện.

Cũng trong ngày đó, những lỗi nghiêm trọng nhất bị bắt nằm ở **giao diện cho máy**, không phải giao diện
đồ hoạ:

- một gate báo xanh khi không chạy gì (#208);
- một cột dữ liệu đo sai đại lượng (#211);
- một bảng quyết định gán nhãn sai (#217);
- một tham số đổi quỹ đạo mà không khai báo (#207, vi phạm [ADR-0008](ADR-0008-world-past-and-time.md)
  động lực 4).

Ngược lại, có những lỗi mà mắt người thấy ngay còn test thì không:

- địa hình bị ép thành bậc thang để có đủ thác nước (#199);
- con mồi lao ra biển tìm thức ăn (F-055).

[ADR-0004](ADR-0004-observer-as-declared-intervention.md) đã xử lý một mặt của câu hỏi này: mọi tác động
của người quan sát lên thế giới, kể cả ánh nhìn qua `LodFocus`, là một can thiệp phải khai báo
(`ObserverPolicy`). ADR này xử lý mặt còn lại: **giao diện được phép làm gì, và lõi mô phỏng nợ giao
diện điều gì.**

Bối cảnh liên quan:

- [kế hoạch Map v2](../ai/requirements/2026-11-20-feature-map-v2.md);
- giao thức NR-SPIKE, so ba ứng viên renderer cho ADR-0011 (chưa có);
- [ghi chú tối ưu thế giới mở](../research/OPEN_WORLD_LOD_STREAMING_RESEARCH_2026-11-19.md), với nguyên tắc
  "mô phỏng không bao giờ phụ thuộc camera".

## Động lực quyết định

- **Khoa học tái lập được mà không cần con người hay màn hình.** Mỗi kết luận phải chạy lại được bằng
  một lệnh và kiểm được bằng một script.
- **Không có thế giới thứ ba.** Không được tách thành bản build cho game và bản build cho nghiên cứu
  (ADR-0004, phương án B đã bị loại).
- **Giao diện không được làm nhiễu quỹ đạo**, trừ những gì ADR-0004 khai báo.
- **Agent là người dùng chính của lõi.** CLI, manifest, artifact và hợp đồng IPC cần được thiết kế,
  đánh phiên bản và có gate như một sản phẩm.
- **Có lỗi chỉ mắt thấy.** Phải có một đường nhìn rẻ để chẩn đoán, tách khỏi giao diện sản phẩm.
- **Giao diện vẫn là sản phẩm với người dùng cuối.** Map v2 và trải nghiệm nhập vai là lý do người ngoài
  mở Anima, nên không được coi giao diện là thứ bỏ đi.

## Các phương án

### Phương án A — Giao diện trước

App desktop là trung tâm. Thí nghiệm chạy trong app, logic quan sát và tổng hợp nằm trong mã frontend.

- **Ưu:** người dùng thấy ngay mọi thứ.
- **Nhược:**
  - kết quả khoa học phụ thuộc vào trạng thái UI và máy có GPU;
  - agent không chạy được;
  - đi ngược hiện trạng: mọi gate của chặng 4 đều chạy không giao diện.
- **Rollback:** đắt.

### Phương án B — Lõi không giao diện; giao diện là client chỉ đọc; giao diện cho máy là hạng nhất *(chọn)*

Lõi mô phỏng và đường thí nghiệm chạy đủ mà không có UI. Renderer, dashboard và app desktop là client:
chúng đọc dữ liệu từ lõi qua các giao diện đã khai báo, và chỉ ghi qua đường can thiệp của ADR-0004. CLI,
schema của manifest, định dạng artifact, hợp đồng IPC và output của bộ kiểm được thiết kế, đánh phiên
bản và có gate như một sản phẩm. Ngoài ra có một tầng "nhìn để chẩn đoán" rẻ, sinh từ artifact.

- **Ưu:**
  - khớp với hiện trạng;
  - giữ tính tái lập;
  - cho phép renderer chạy ở tiến trình riêng (ứng viên (c) của NR-SPIKE);
  - đổi renderer không đụng tới khoa học.
- **Nhược:**
  - mỗi thứ UI cần phải đi qua một giao diện khai báo, không đọc thẳng nội bộ ECS;
  - cần duy trì thêm một tầng chẩn đoán.
- **Rollback:** không cần. Đây là mô tả và siết lại hiện trạng.

### Phương án C — Không giao diện

Bỏ app desktop; Anima chỉ là thư viện và CLI nghiên cứu.

- **Ưu:** đơn giản nhất.
- **Nhược:**
  - mất sản phẩm (Map v2, nhập vai, chặng 5 và 6);
  - mất đường nhìn để bắt lỗi chỉ mắt thấy.
- **Rollback:** đắt.

### Giữ nguyên hiện trạng (không ghi thành quy tắc)

Hiện trạng gần với phương án B, nhưng chưa có quy tắc nào được ghi lại. Một agent có thể đặt logic tổng
hợp hay quyết định mô phỏng vào mã TypeScript, hoặc cho LOD phía render ảnh hưởng ngược vào mô phỏng,
mà không vi phạm văn bản nào.

## Quyết định

Chọn **phương án B**, gồm sáu quy tắc.

**U1 — Lõi chạy đủ không cần giao diện.**
- Mọi kết luận khoa học (gate, kết quả thí nghiệm, số đo trong STATE §1) phải tái lập được bằng lệnh
  dòng lệnh, không mở app và không cần GPU.
- Đầu ra là artifact có checksum, kèm một bộ kiểm độc lập khi kết luận đi vào bản đăng ký hay bài báo.

**U2 — Giao diện là client chỉ đọc.**
- App desktop, renderer (Three.js hôm nay, ứng viên của ADR-0011 sau này) và dashboard chỉ đọc trạng
  thái mô phỏng qua các giao diện đã khai báo: sự kiện IPC trong
  [`IPC_CONTRACT.md`](../reference/IPC_CONTRACT.md), artifact, và world artifact ghim của
  [ADR-0010](ADR-0010-pinned-world-artifact.md).
- Mọi đường ghi từ giao diện vào thế giới là can thiệp theo ADR-0004 (lệnh IPC có khai báo, `CauseId`,
  `ObserverPolicy`).
- Mã frontend không được chứa luật mô phỏng, phép tính sinh thái hay quyết định tiến hoá. Nó chỉ được
  trình bày, nội suy để hiển thị, và gửi lệnh đã khai báo.

**U3 — Tối ưu phía render không chạm vào mô phỏng.**
- LOD, culling, streaming, nội suy tư thế và giảm tần suất gửi IPC cho vật thể ở xa đều nằm hoàn toàn ở
  phía render.
- Ngoại lệ duy nhất là cơ chế đã được ADR-0004 khai báo (`LodFocus` theo `ObserverPolicy`), và khi nó tắt
  thì lượt chạy giống từng bit với lượt chạy không có giao diện.

**U4 — Giao diện cho máy là hạng nhất.**
- CLI (`anima-lab`, các example runner), schema manifest, định dạng artifact và JSON báo cáo, hợp đồng
  IPC, và output của bộ kiểm được:
  - ghi trong [`docs/reference/`](../reference/README.md);
  - đánh phiên bản: đổi định dạng thì tăng số phiên bản và có đường đọc dữ liệu cũ;
  - giữ bằng gate có đối chứng âm, như `check:ipc` và `check_ipc_reference.mjs` đang làm với IPC.
- Một thay đổi giao diện cho máy cần cùng mức review như một thay đổi luật mô phỏng.

**U5 — Tầng nhìn để chẩn đoán, tách khỏi giao diện sản phẩm.**
- Sinh từ artifact, không từ app đang chạy: ảnh chụp bản đồ chuẩn (skill `map-visual-check`), biểu đồ
  quần thể và nguyên nhân chết, biểu đồ Muller, ảnh lớp địa hình của World v2.
- Được phép thô. Khi một thẻ đổi thứ có thể nhìn thấy (địa hình, hành vi, sinh vật), bằng chứng nộp kèm
  có ít nhất một ảnh chẩn đoán trước và sau.

**U6 — Giao diện sản phẩm là đường song song, không chặn khoa học.**
- Map v2, renderer và dashboard tiến theo kế hoạch riêng (QD-MAPV2), và không bao giờ là điều kiện tiên
  quyết của một cổng chặng 4.
- Khi chọn renderer (ADR-0011), tính "chỉ là client" là tiêu chí: một ứng viên đòi đưa logic mô phỏng
  vào renderer, hoặc ghi ngược trạng thái ngoài ADR-0004, bị loại.

## Hệ quả

- **Tích cực:**
  - tách rõ ranh giới để agent không đặt nhầm chỗ;
  - đổi renderer an toàn;
  - giao diện cho máy được đầu tư đúng mức.
- **Chi phí:**
  - một thứ UI muốn hiển thị phải có giao diện khai báo trước, nên đôi khi chậm hơn đọc tắt;
  - phải duy trì tầng chẩn đoán.
- **Hiện trạng cần rà lại** (không chặn ADR này):
  - mã frontend hiện có tính toán nào là luật mô phỏng không;
  - dashboard có đọc gì ngoài sự kiện IPC đã khai báo không.
  Việc rà là một thẻ riêng, nếu chủ dự án muốn.

## Kiểm chứng

- **U1:** mỗi kết quả trong STATE §1 và mỗi `RESULT.md` có lệnh tái lập không giao diện. Bộ kiểm
  `verify_*_artifacts` chạy trong CI cho các thí nghiệm đã đăng ký.
- **U2, U4:** `check:ipc` và `check_ipc_reference.mjs` (đã có) giữ hợp đồng IPC. Một gate tĩnh cho U2
  (ví dụ cấm frontend import module mô phỏng) để dành cho thẻ rà soát ở mục Hệ quả.
- **U3:** test "bật và tắt ghi cho cùng checksum" của ADR-0008, cùng test `ObserverPolicy::Absent`
  giống từng bit của ADR-0004.
- **U5:** review của lead đòi ảnh chẩn đoán cho thẻ đổi thứ nhìn thấy được.

## Nguồn gốc quyết định

Chủ dự án hỏi trong chat ngày 2026-11-23: "giao diện chỉ là thứ dùng cho con người, còn phần không thấy
không cần giao diện?". Lead trả lời: đồng ý phần lớn, với ba điều chỉnh, là U4, U5 và U6 ở trên. Chủ dự án
đồng ý và yêu cầu ghi thành ADR: "oke viết ADR đó đi".
