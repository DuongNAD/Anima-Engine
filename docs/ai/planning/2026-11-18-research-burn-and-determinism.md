---
phase: planning
feature: release-licensing-and-determinism
title: Hai báo cáo nghiên cứu về nâng Burn 0.22 và tất định số học giữa các máy, đối chiếu với mã
description: Đọc và đối chiếu hai báo cáo Gemini Deep Research ngày 2026-11-18. Ghi hai quyết định của chủ dự án cho thẻ R1c (QĐ-R1c-a schema 8 và khởi tạo lại learner; QĐ-R1c-b chuyển sang burn-flex) và một phương án cụ thể cho QĐ-M6, chưa quyết
status: active
owner: simulation-architecture
last_reviewed: 2026-11-18
---

# Nâng Burn 0.22 và tất định số học: đối chiếu hai báo cáo nghiên cứu

## 1. Nguồn và độ tin cậy

Hai báo cáo do Gemini Deep Research soạn ngày 2026-11-18, trả lời câu 4 và câu 2 của bộ prompt nghiên cứu (xem
phụ lục của [đề xuất làm lại SEL2](2026-11-18-sel2-redesign-proposal.md#phụ-lục-prompt-đã-dùng)). Chủ dự án giữ bản
gốc; repo không lưu chúng.

| Báo cáo | Câu hỏi | Độ tin cậy |
|---|---|---|
| *Burn 0.22 Upgrade Analysis* | Câu 4: nâng `burn` 0.13.2 lên 0.22 | Khá. Phần lớn trích docs.rs, blog phát hành của Tracel và issue GitHub. Không kiểm lại được trong lượt này vì các bản phát hành đều sau tháng 6/2026 |
| *Deterministic Rust Floating-Point Simulation* | Câu 2: tái lập bit giữa Windows, Linux, macOS | Vừa. Phần cơ chế (IEEE-754, FMA, thư viện toán) có nguồn tốt; các tỉ lệ lệch và số tick tới lúc lệch trích về `unknown_url`, tức mô hình riêng của Gemini, nên **không** được dùng ở đây |

Cả hai đã được đọc hết và đối chiếu với mã trên thân cây tại `8c7c2c7`.

## 2. Nâng Burn (thẻ R1c)

### 2.1 Báo cáo nói gì, và mã nói gì

| Khẳng định | Đối chiếu | Dùng thế nào |
|---|---|---|
| `hexf-parse` rời cây: naga 30 → wgpu-core 30 → wgpu 30 → cubecl-wgpu 0.11.4 → burn-wgpu 0.22.1 | Khớp với điều QĐ-C đã dựa vào; chưa phân giải thật | Bước 0 của R1c vẫn kiểm bằng một lần phân giải |
| naga 30 có một bộ đọc số thực hex nội tuyến mang ghi chú giấy phép 0BSD | 0BSD là giấy phép dễ dãi; script giấy phép của dự án đọc theo crate, không theo file | Ghi trong ADR của R1c |
| Burn 0.22 đòi Rust 1.95 trở lên, edition 2024 | `rust-toolchain.toml` ghim 1.98.1 | Không chặn |
| `BinBytesRecorder` của 0.13 **không** nạp được ở 0.22; định dạng gãy ở 0.14, 0.16, 0.21, và 0.22 bỏ hẳn các recorder cũ. Hướng dẫn chuyển đổi chính thức chỉ chuyển trọng số, bỏ trạng thái optimizer | Snapshot schema 7 lưu learner dùng chung bằng hai bản ghi `BinBytesRecorder` (`SavedLearnerWorkerState`) | Quyết định QĐ-R1c-a ở §2.2 |
| `burn-ndarray` bị khai tử ở 0.21 và đã bị xoá ở 0.23 (bản thử). Thay thế là `burn-flex`, một backend CPU thuần Rust | Dự án dùng `burn-ndarray` | Quyết định QĐ-R1c-b ở §2.2 |
| Tham số kiểu backend bị bỏ khỏi mã người dùng; autodiff thành ngữ cảnh lúc chạy (`device.autodiff()`, `model.valid()`); `AdamConfig::init()` trả `ModuleOptimizer` | `ai/model.rs` và `core/training.rs` dùng các kiểu của 0.13 | Phạm vi sửa nằm trong dòng uỷ quyền của R1c |
| `to_device` **giữ** autodiff; suy luận phải gọi `.valid()` hoặc `.without_autodiff()`, nếu không băng ngược tích luỹ | Đường suy luận của learner dùng chung | Ghi vào prompt R1c như một bẫy |
| `burn-flex` mặc định số nguyên I32, `burn-ndarray` I64 | Chưa kiểm chỗ nào dùng tensor số nguyên | Ghi vào prompt R1c |
| `Device::cpu()` (CubeCL JIT) có gộp phép tính và tự chọn lệnh theo máy; dùng `burn-flex` cho tất định | — | Ghi vào prompt R1c |
| Chỗ rò của burn-wgpu 0.13 (burn#2042) không còn tái hiện; còn một lỗi mở (burn#5891) chỉ gặp với driver NVIDIA 615.71 trên Linux | Thẻ R1c điều kiện 5 đo lại chỗ rò | Đo vẫn bắt buộc; GPU giữ tắt mặc định |
| Quỹ đạo CPU sẽ khác sau khi nâng: `rand` 0.10, nhân GEMM mới, các sửa lỗi NaN | Điều kiện 4 của R1c đã dự kiến | `LIVE_MODEL_VERSION` vẫn do chủ dự án quyết |

### 2.2 Quyết định của chủ dự án (2026-11-18, "Theo đề xuất")

| ID | Quyết định | Vì sao |
|---|---|---|
| **QĐ-R1c-a** | Tăng `SCHEMA_VERSION` từ 7 lên 8. Một snapshot schema 7 vẫn khôi phục được thế giới, nhưng hai bản ghi learner của nó bị từ chối kèm một lỗi có tên và một cảnh báo, và learner dùng chung khởi tạo lại từ đầu | Đọc bản ghi cũ đòi giữ `burn` 0.13 trong lockfile dưới tên khác, và `burn-core` 0.13 kéo lại `zip` 0.6.6 cùng các advisory đang phải miễn. Burn tự nó cũng không chuyển trạng thái optimizer qua ranh giới 0.22 |
| **QĐ-R1c-b** | R1c chuyển thẳng sang `burn-flex`, không qua `burn-ndarray` 0.22 | `burn-ndarray` đã bị xoá ở 0.23; giữ nó nghĩa là phải đổi backend thêm một lần nữa |

Hai quyết định đều đổi quỹ đạo và định dạng lưu. Không quyết định nào ở đây nói gì về `LIVE_MODEL_VERSION`; điều
kiện 4 của R1c vẫn hỏi chủ dự án.

## 3. Tất định số học giữa các máy (đầu vào cho QĐ-M6)

### 3.1 Báo cáo nói gì, và mã nói gì

| Khẳng định | Đối chiếu | Kết luận |
|---|---|---|
| Cộng, trừ, nhân, chia và `sqrt` được IEEE-754 làm tròn đúng, nên giống nhau trên mọi máy; các hàm siêu việt của hệ điều hành thì không | Khớp với ma trận M1: `sqrt` khớp, sáu hàm siêu việt lệch 1 ULP | Khớp |
| `ln` cũng không được làm tròn đúng trên cả ba hệ điều hành; M1 thấy nó khớp chỉ nhờ dữ liệu đầu vào lần đó | M1 chỉ đo trên đầu vào của một lượt chạy | **Rủi ro tiềm ẩn mới.** Không coi `ln` là an toàn |
| Rust mặc định **không** gộp nhân-cộng (FMA); chỉ `mul_add`, các hàm `algebraic_*` hoặc cờ `-fp-contract=fast` mới gộp | Mã engine không gọi `mul_add` | Gợi ý `-C llvm-args=-fp-contract=off` của báo cáo hiệu năng trước là không cần |
| Mã engine gọi hàm toán của hệ điều hành | Đếm trên thân cây (`git grep`, chỉ `src/` và `crates/`): `sin` 20, `cos` 5, `exp` 5, `powf` 7, `ln` 3; `powi` 16 (phần lớn là bình phương); không có `tanh` hay `atan2` | Khoảng 40 chỗ cần chuyển |
| Viết vòng tính mạng nơ-ron riêng, không cấp phát, thứ tự cộng cố định | Não tiến hoá (`BrainGenotype::forward_into`) đã là vòng tính tự viết như vậy, nhưng gọi `exp` của hệ điều hành cho sigmoid. Não dùng chung chạy qua Burn | Một nửa đã có |
| Nhân ma trận của Burn chọn lệnh SIMD theo máy, nên lệch giữa x86 và ARM | Não dùng chung chạy qua Burn | Não dùng chung không tái lập được giữa các máy nếu không đổi đường suy luận |
| Thư viện `libm` thuần Rust, ghim đúng phiên bản, tắt tính năng `arch`, cho kết quả như nhau trên x86 và ARM; `core-math` cho làm tròn đúng nhưng cần trình biên dịch C | Dự án chưa dùng crate nào trong hai crate này | Ứng viên chính |
| `clippy.toml` với `disallowed-methods` cấm gọi thẳng các hàm toán của hệ điều hành, như Bevy làm | Dự án chưa có `clippy.toml` | Một gate rẻ |

### 3.2 Một phương án cụ thể cho QĐ-M6 (chưa quyết)

QĐ-M6 hỏi mức tất định giữa các nền tảng. Nếu chủ dự án chọn "tái lập bit giữa các máy", phương án rẻ nhất mà hai
báo cáo và mã cùng chỉ ra là:

1. Một module `sim_math` trong `anima-domain` bọc crate `libm` (ghim đúng phiên bản, `default-features = false`);
   tất cả khoảng 40 chỗ gọi trên đi qua nó, kể cả `exp` trong `BrainGenotype::forward_into`.
2. Một `clippy.toml` cấm `f32::sin`, `cos`, `exp`, `powf`, `ln` và họ hàng, cùng `mul_add` và các hàm
   `algebraic_*`, để chúng không quay lại.
3. Ma trận `.github/workflows/determinism-matrix.yml` (thẻ M1) là phép đo: nó phải hết lệch trên thế giới sống.
4. Não dùng chung chạy qua Burn là quyết định riêng: hoặc chấp nhận nó là ngoại lệ (nó là đường mặc định cũ), hoặc
   chuyển suy luận của nó sang vòng tính tự viết như não tiến hoá.

Phương án này đổi quỹ đạo của mọi thế giới, nên nó cần một quyết định về `LIVE_MODEL_VERSION` và chỉ nên làm sau
R1c, vì R1c cũng đổi quỹ đạo.

## 4. Không dùng làm bằng chứng

- Tỉ lệ "74–86 % tích vô hướng lệch", số tick tới lúc lệch, và mọi số khác trích về `unknown_url`.
- Các tỉ lệ lệch của dự án Verse và LarmorX: có nguồn nhưng là phép đo trên hệ thống khác.
- Bảng giá runner GitHub và lịch ngừng hỗ trợ ảnh runner: phụ thuộc thời điểm, không kiểm lại.
