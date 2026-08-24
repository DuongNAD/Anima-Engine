---
title: ADR-0001 — Tách development khỏi spawn và lưu phenotype
status: accepted
owner: simulation-architecture
last_reviewed: 2026-07-24
decision_date: 2026-07-24
supersedes: none
superseded_by: none
---

# ADR-0001 — Tách development khỏi spawn và lưu phenotype

## Bối cảnh

`decode_genotype` hiện vừa diễn giải genotype, vừa tạo hình học, khởi tạo homeostasis
và spawn ECS entities. Hàm được dùng cho genesis, epoch replacement, restore save và
migration. Nếu đọc môi trường trực tiếp trong hàm này, cá thể restore/migration bị
“phát triển lại”, trong khi developmental plasticity chỉ hợp lệ lúc hình thành cá thể.

Bản kế hoạch Claude cũ còn:

- suy `LocomotionMedium` từ cùng environment dùng để validate spawn;
- áp Bergmann trong cả prior và decode;
- không lưu phenotype;
- gọi local trait/biome correlation là S43;
- giả định epoch replacement là reproduction có parent sống.

## Động lực quyết định

- Restore/migration phải giữ identity và phenotype của cùng cá thể.
- Development chỉ chạy một lần và tái lập theo seed.
- Habitat legality phải so trait sinh vật với environment độc lập.
- Save cũ phải load mà không tự đổi ngoại hình.
- Geometry, physics, renderer và MAP-Elites phải đọc cùng phenotype.
- Năng lượng của mọi spawn phải có source/sink.

## Các phương án

### A. Đọc environment ngay trong `decode_genotype`

Ít thay call-site nhưng trộn birth với restore/migration, khó version phenotype và dễ
double-apply. **Bị từ chối.**

### B. Luôn suy phenotype từ genotype + environment hiện tại

Không cần lưu phenotype nhưng cá thể đổi hình khi di chuyển hoặc khi thuật toán nâng
version. Không thể bảo toàn save/migration. **Bị từ chối.**

### C. Tách `develop_at_birth` và `spawn_developed`, lưu phenotype

Thêm data/migration work nhưng semantics rõ, test được và hỗ trợ versioning. **Được chọn.**

### D. Chỉ dùng genotype, không có plasticity

Đơn giản và tái lập nhưng không đáp ứng mục tiêu cùng genotype phát triển khác nhau
theo môi trường. Có thể là feature-flag fallback, không phải hướng chính.

## Quyết định

1. Tạo module `evolution/ecomorph.rs` chứa sampling và development thuần.
2. Tạo `DevelopedPhenotype` có version và serialize nó trong save/migration.
3. Tách development khỏi ECS spawn theo API trong
   [`CREATURE_DEVELOPMENT_CONTRACT.md`](../reference/CREATURE_DEVELOPMENT_CONTRACT.md).
4. Chỉ genesis/birth/intervention được develop; restore/migration spawn bản đã lưu.
5. Habitat/medium là trait của sinh vật.
6. Reaction norm strength là trait có bound; default `0` giữ hành vi save cũ.
7. Genesis prior chỉ bias genotype distribution một lần, không nhân lại phenotype.
8. S43 giữ nguyên nghĩa Red-Queen; local adaptation dùng CM-S11 reciprocal transplant.
9. Epoch replacement được ghi nhãn riêng, không gọi là biological birth.

## Hệ quả

### Tích cực

- Save/load và migration giữ cùng cá thể.
- Có một representation phenotype chung cho physics, selection và renderer.
- Plasticity có thể tiến hóa thay vì là lợi ích miễn phí giống nhau cho mọi genotype.
- Dễ benchmark, feature flag và rollback.

### Tiêu cực / rủi ro

- Save/migration schema lớn hơn.
- Phải cập nhật bốn call-site và cross-language types.
- Cần migration cho save cũ.
- Cần giải quyết duplicate frontend model (`src/types/index.ts` và `src/App.tsx`).

## Kế hoạch triển khai và hoàn tác

1. Thêm type + pure functions và test, chưa đổi runtime.
2. Thêm phenotype serialization với default/migration.
3. Chuyển từng call-site; restore/migration trước để khóa invariant.
4. Bật feature flag cho genesis/replacement.
5. Chỉ bật mặc định sau CM-S01…CM-S10 và map review.
6. Rollback bằng cách tắt development nhưng vẫn giữ reader phenotype/version.

## Bằng chứng xác minh

| Gate | Artifact | Ngưỡng | Trạng thái |
|---|---|---|---|
| Correctness | CM-S01…CM-S10 | 0 fail | pending |
| Determinism | CM-S02/CM-S05 | exact/tolerance đã chốt | pending |
| Save/migration | CM-S03/CM-S04 | không đổi phenotype | pending |
| Energy | CM-S08 | delta trong S01 tolerance | pending |
| Performance | spawn + tick benchmark | tick không allocation mới | pending |
| Map/ecology | Animal Map Vision spawn view | 0 critical/high | blocked: MCP unavailable |

## Tài liệu bị ảnh hưởng

- Contract: [`CREATURE_DEVELOPMENT_CONTRACT.md`](../reference/CREATURE_DEVELOPMENT_CONTRACT.md)
- Explanation: [`CREATURE_MORPHOGENESIS.md`](../explanation/CREATURE_MORPHOGENESIS.md)
- Planning: [`CREATURE_MORPHOGENESIS_PLAN.md`](../planning/CREATURE_MORPHOGENESIS_PLAN.md)
- Roadmap: M5/M7 trong [`WORLD_SIMULATION_PLAN.md`](../../WORLD_SIMULATION_PLAN.md)
