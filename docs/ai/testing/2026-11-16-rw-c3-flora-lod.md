---
phase: testing
feature: readable-world
title: Testing — RW-C3 cây cỏ theo khoảng cách và mật độ theo mức
description: Số tam giác và lệnh vẽ trước/sau RW-C3 ở ba tư thế × ba mức, đo bằng bộ đếm RW0-4
status: proposed
owner: maintainers
last_reviewed: 2026-11-16
---

# RW-C3: số đo trước và sau

Đo bằng `window.__animaFrameCounters` (RW-C1) qua Playwright Chromium (ANGLE/Metal, `MacBook Air M5`),
khung `1024×768`, `landscape.html?quality=<mức>` với tư thế ghim bằng `__animaSetCanonicalPose`.
Cột "1000" dùng `&replay=synthetic&agents=1000`. Số tam giác và lệnh vẽ không phụ thuộc GPU; bản đo
"trước" trên Mac khớp từng chữ số với bản đo Dell tại `bb78e63d` (3.256.360 tam giác cây cỏ). **Chưa đo FPS**:
đó là việc của RW6 trên Dell.

Định dạng ô: tổng tam giác / tam giác cây cỏ / lệnh vẽ cây cỏ / lệnh vẽ tổng.

| Sinh vật | Mức | Tư thế | Trước | Sau |
|---|---|---|---|---|
| 0 | Thấp | overview | 3.617.258 / 3.256.360 / 14 / 30 | 437.032 / 76.134 / 1 / 17 |
| 0 | Thấp | ecosystem | 3.617.258 / 3.256.360 / 14 / 30 | 601.956 / 241.058 / 13 / 29 |
| 0 | Thấp | spawn | 3.617.258 / 3.256.360 / 14 / 30 | 521.236 / 160.338 / 13 / 29 |
| 0 | Vừa | overview | 3.617.258 / 3.256.360 / 14 / 30 | 513.232 / 152.334 / 1 / 17 |
| 0 | Vừa | ecosystem | 3.617.258 / 3.256.360 / 14 / 30 | 941.354 / 580.456 / 15 / 31 |
| 0 | Vừa | spawn | 3.617.258 / 3.256.360 / 14 / 30 | 779.862 / 418.964 / 13 / 29 |
| 0 | Cao | overview | 10.671.260 / 10.303.774 / 21 / 41 | 676.864 / 309.378 / 1 / 21 |
| 0 | Cao | ecosystem | 10.671.260 / 10.303.774 / 21 / 41 | 2.739.712 / 2.372.296 / 22 / 41 |
| 0 | Cao | spawn | 10.671.190 / 10.303.774 / 21 / 40 | 2.203.986 / 1.836.570 / 20 / 39 |
| 1000 | Thấp | overview | 3.619.258 / 3.256.360 / 14 / 31 | 439.032 / 76.134 / 1 / 18 |
| 1000 | Thấp | ecosystem | 4.165.730 / 3.256.360 / 14 / 34 | 1.150.428 / 241.058 / 13 / 33 |
| 1000 | Thấp | spawn | 4.102.680 / 3.256.360 / 14 / 35 | 1.006.658 / 160.338 / 13 / 34 |
| 1000 | Vừa | overview | 3.619.258 / 3.256.360 / 14 / 31 | 515.232 / 152.334 / 1 / 18 |
| 1000 | Vừa | ecosystem | 4.165.730 / 3.256.360 / 14 / 34 | 1.489.826 / 580.456 / 15 / 35 |
| 1000 | Vừa | spawn | 4.102.680 / 3.256.360 / 14 / 35 | 1.264.744 / 418.964 / 13 / 34 |
| 1000 | Cao | overview | 10.673.260 / 10.303.774 / 21 / 42 | 678.864 / 309.378 / 1 / 22 |
| 1000 | Cao | ecosystem | 11.427.684 / 10.303.774 / 21 / 45 | 3.496.136 / 2.372.296 / 22 / 45 |
| 1000 | Cao | spawn | 11.340.504 / 10.303.774 / 21 / 45 | 2.872.540 / 1.836.570 / 20 / 44 |

So với §4a: cây cỏ Thấp ≤ 530.000 (≤ 241.058), Vừa ≤ 1.730.000 (≤ 580.456), Cao ≤ 5.030.000 gồm bóng đổ
(≤ 2.372.296); lệnh vẽ cây cỏ ≤ 16 / 16 / 23 (lớn nhất 13 / 15 / 22); tổng lệnh vẽ ≤ 60 / 60 / 70 (lớn nhất
34 / 35 / 45). Tổng tam giác với 1000 sinh vật ở `ecosystem`: Thấp 1.150.428 (≤ 1.200.000), Vừa 1.489.826
(≤ 2.500.000), Cao 3.496.136 (≤ 6.000.000). Lệnh vẽ tăng so với trước không quá 1 (mesh impostor).

Chế độ chụp (`?capture=1`) vẫn đi đường vẽ cũ: cây cỏ `10.303.774` tam giác, `21` lệnh, ở cả `overview` và
`spawn`, bằng bản đo trước.

Chưa kiểm: FPS trên mọi máy; so sánh điểm ảnh 8 ảnh chuẩn (đường vẽ cũ giữ nguyên mã, chỉ bộ tô màu được tách
thành hàm dùng chung).
