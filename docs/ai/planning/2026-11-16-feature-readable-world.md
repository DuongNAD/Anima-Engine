---
phase: planning
feature: readable-world
title: Kế hoạch — Thế giới nhìn rõ: sinh vật và bản đồ đẹp, nhẹ, mượt
description: Vẽ sinh vật bằng khối ghép theo đốt thật với chuyển động lấy từ mô phỏng, giữ bản đồ low-poly, ba mức chất lượng tự chọn theo máy, và ngân sách đo được cho từng mức
status: proposed
owner: simulation-architecture
last_reviewed: 2026-11-16
---

# Kế hoạch — Thế giới nhìn rõ

Yêu cầu của chủ dự án (2026-11-16): bản đồ và sinh vật phải **đẹp, đủ nhẹ và mượt**, chạy được trên
nhiều máy, kể cả máy yếu.

Hướng đi: giữ phong cách low-poly tô màu phẳng mà bản đồ đang dùng. Sinh vật được vẽ bằng các khối
ghép theo **đúng từng đốt thật**, chuyển động lấy từ **chính mô phỏng**. Cảnh có ba mức chất lượng,
tự chọn theo máy, mỗi mức có ngân sách kiểm được bằng gate.

Chưa có dòng mã nào của kế hoạch này được viết. Mọi số ở §2 là số đo; mọi số ở §4 là đề xuất.

## 1. Mục tiêu và phạm vi

| Mục tiêu | Nghĩa là gì, đo bằng gì |
|---|---|
| **Nhìn rõ và đẹp** | Nhìn một con là biết nó thuộc loại nào, to nhỏ ra sao, quay hướng nào, khoẻ hay đói, kể cả khi ảnh chuyển sang thang xám. Ban đêm vẫn thấy địa hình và sinh vật. Kiểm bằng ảnh chụp ở các tư thế camera chuẩn |
| **Mượt** | Chuyển động liên tục ở 60 khung hình dù backend chỉ phát 30 gói mỗi giây (`EMIT_INTERVAL`, `src-tauri/src/core/emit.rs:28`). Kiểm bằng test lấy mẫu ở 30/60/144 Hz |
| **Nhẹ** | Mỗi mức chất lượng có trần tam giác và lệnh vẽ mỗi khung (§4), kiểm bằng gate. FPS thật đo trên từng máy (§5, RW6) |

**Trong phạm vi:** cảnh 3D `landscape.html` (`src/components/Landscape/**`), khung 2D
(`src/PixiViewport.tsx`), test và spec đi kèm. Toàn bộ là giao diện.

**Ngoài phạm vi:**

- Mọi thay đổi trong `src-tauri/src`. Hai việc cần đụng backend được tách thành quyết định ở §8.
- Mesh liền có xương, mesh do AI tạo, CPG ghép nối, IK (các nhánh A2–A4, N2–N4 của
  [task backlog Thế giới sống](../implementation/2026-08-25-feature-living-world-tasks.md)).
- Sinh địa hình, biome, thuỷ văn. Kế hoạch này không đổi dữ liệu thế giới.
- Đổi renderer (WebGPU, engine khác).

## 2. Hiện trạng đo ngày 2026-11-16

Đo trên `landscape.html` qua `npm run dev`, không có backend, trong khung trình duyệt 1024×768 điểm
CSS, trên MacBook Air M5 (không quạt). Số tam giác và lệnh vẽ lấy bằng cách bọc các lời gọi `draw*` của WebGL trong 3
giây; chúng gồm cả lượt vẽ bóng đổ.

| Hạng mục | Số đo | Ghi chú |
|---|---|---|
| Mức "Đẹp" | 40 lệnh vẽ, 10.671.190 tam giác mỗi khung, 60 FPS | Bộ đệm 1536×1152 (DPR 1,5) |
| Mức "Nhẹ" | 33 lệnh vẽ, 3.623.776 tam giác mỗi khung, 60 FPS | Bộ đệm **vẫn** 1536×1152 |
| Phân bố ở mức "Nhẹ" | Năm lệnh vẽ instanced lớn nhất: 2.860.814 tam giác (79%). Địa hình: 294.912 (8%) | Năm lệnh đó có 3.448–13.808 instance mỗi lệnh, khớp với thảm thực vật; chưa gắn nhãn từng lệnh |
| Phụ thuộc camera | Cùng số tam giác ở chế độ Quay và Đi Bộ | Cây cỏ được vẽ ở mọi khoảng cách |
| Độ dài một ngày | 48 giây | `WorldShowcase.tsx:267`: cộng `0.05 × speed` giờ mỗi 100 ms |
| Ban đêm | Từ 19:30 tới 4:30, `sunIntensity = 0`, `ambientIntensity = 0.05` | `utils/skyParams.ts:24-32`. Ảnh chụp lúc 20:06 gần như đen hoàn toàn |

Đọc từ mã, cùng ngày:

- **Sinh vật 3D** (`LiveAgents.tsx`): mỗi đốt là một quả cầu bán kính cố định `SEGMENT_RADIUS = 1.6`
  (dòng 44, 77). Chỉ dùng `x`, `z` và `yaw`. Độ cao lấy từ địa hình (dòng 142). Màu theo `agent_type`,
  độ sáng theo năng lượng (dòng 157). Các đốt không nối với nhau.
- **Sinh vật 2D** (`PixiViewport.tsx:702-760`): toàn màu xám; mỗi **đốt** của con mồi được vẽ như một
  con riêng, có thanh nước riêng.
- **Dữ liệu đã có mà chưa dùng** (`src/types/generated/SegmentState.ts`): `length`, `radius`, `y`,
  `pitch`, `roll`, `parent_segment_id`, toạ độ và trục khớp, `head_direction`.
- **Chuyển động thật có trong mô phỏng**: `resolve_joints_system` xoay đốt con quanh trục khớp theo
  `cpg.output` (`src-tauri/src/physics/dynamics.rs:53`). Góc được phát theo thứ tự Euler `YXZ`
  (`src-tauri/src/core/simulation_loop.rs:2034`).
- **Thú trang trí** (`WorldWildlife.tsx`) đã làm đúng kỹ thuật cần dùng: ghép vài khối, tô màu đỉnh,
  cả đàn một lệnh vẽ.
- **Đã có sẵn từ commit `970d963`**: `length` và `radius` trong payload (A1-1 tới A1-3), và hai hàm
  nội suy thuần `lerpVec3`, `lerpAngle` (`agentInterpolation.ts`, N1-1). `LiveAgents.tsx` chưa gọi
  chúng. Task backlog vẫn ghi các mục này là ⬜.

Giới hạn của số đo: MacBook Air M5 là máy mạnh, nên 60 FPS ở đây **không** nói gì về máy yếu. Chưa có
số đo nào trên máy khác.

## 3. Phương án đã chọn

**Khối ghép theo đốt thật, chuyển động từ mô phỏng.** Mỗi đốt là một hình trụ đúng `length` và
`radius`, khớp là hình cầu, đặt theo đúng vị trí và hướng mà backend phát. Mọi đốt của mọi con vẽ
chung vài lệnh instanced. Giữa hai gói tick thì nội suy.

| Phương án | Vì sao chọn hoặc loại |
|---|---|
| Chỉ biểu tượng 2D | Rõ và rẻ nhất, nhưng mất hình dáng và chuyển động. Giữ lại làm lớp nhìn từ xa (RW2-6) |
| **Khối ghép theo đốt thật** | Dữ liệu đã có sẵn; đúng với mọi hình thái mà tiến hoá tạo ra; số lệnh vẽ không tăng theo số con; thứ nhìn thấy là thứ mô phỏng tính |
| Mesh liền có xương (A2) | Mượt hơn ở cận cảnh, nhưng mỗi hình thái một mesh, phải sinh lại khi gen đổi, khó vẽ instanced. Để sau, cho con đang được chọn |
| Mesh do AI tạo (A3, A4) | Cần dịch vụ ngoài, tốn phí, khó khớp với hình thái thay đổi |
| WebGPU hoặc đổi engine | App chạy trên webview của từng hệ điều hành, không phải webview nào cũng có WebGPU. Viết lại renderer không giải quyết yêu cầu nào ở §1 |

Hai nguyên tắc đi kèm:

- **Phân biệt bằng hình trước, màu sau.** Kẻ săn và con mồi khác hình đầu, nên người mù màu và ảnh
  thang xám vẫn đọc được.
- **Giao diện không bịa chuyển động có nghĩa.** Thứ thêm ở giao diện (thở, nhún, mờ dần) chỉ là
  trang trí, tất định theo `agent_id`, và không bao giờ thay cho tư thế mà backend phát. Đây là ràng
  buộc §2 của [thiết kế animation](../design/2026-08-25-feature-creature-animation.md).

## 4. Ngân sách hiệu năng (đề xuất)

Số mỗi khung, tính cả lượt bóng đổ, đo ở ba tư thế camera chuẩn `overview`, `ecosystem` và `spawn`
(`CANONICAL_VIEW_CAMERAS`), với fixture 1000 con × 3 đốt. Các trần này là **đề xuất**; RW0-5 chốt
chúng bằng số đo.

| Mức | Tam giác tổng | Trong đó sinh vật | Lệnh vẽ | DPR | Bóng đổ | Cây cỏ | Mục tiêu FPS |
|---|---|---|---|---|---|---|---|
| Thấp | ≤ 1,2 triệu | ≤ 300 nghìn | ≤ 60 | 0,75–1 | tắt, dùng bóng tròn giả | 1/4, tầm gần | 30 trên GPU tích hợp (Dell Vostro 3530) |
| Vừa | ≤ 2,5 triệu | ≤ 400 nghìn | ≤ 60 | 1 | tắt, dùng bóng tròn giả | 1/2 | 60 |
| Cao | ≤ 6 triệu | ≤ 600 nghìn | ≤ 70 | tới 1,5 | bật | đủ | 60 trên GPU rời (RTX 5060 Ti của máy case) hoặc Apple Silicon |
| Hôm nay "Nhẹ" | 3,6 triệu | — | 33 | 1,5 (lỗi) | tắt | 1/2 | |
| Hôm nay "Đẹp" | 10,7 triệu | — | 40 | 1,5 | bật | đủ | |

Sinh vật không được vượt 5 lệnh vẽ ở bất kỳ số con nào: thân trụ, cầu khớp và mắt, mõm, bóng tròn,
dấu nhìn từ xa.

## 5. Các gói việc

Mỗi gói là một PR, chỉ sửa giao diện và test.

### RW0 — Dụng cụ đo và phát lại · làm trước

| ID | Việc | Neo | Xong khi |
|---|---|---|---|
| RW0-1 | Tách nguồn tick thành hai cài đặt: sự kiện Tauri `simulation-tick` (như hiện nay) và phát lại từ một dãy payload | `LiveAgents.tsx` (khối `listen`) | Vitest: cùng một dãy payload đi qua hai nguồn cho cùng dãy tư thế |
| RW0-2 | Fixture tổng hợp tất định: hàm thuần sinh N con, 3–15 đốt, đi theo quỹ đạo, khớp vung theo sin. Mở bằng `landscape.html?replay=synthetic&agents=N` | file mới `utils/syntheticTicks.ts` | Cùng tham số cho cùng JSON; trình duyệt thường hiện sinh vật mà không cần backend. Mã ghi rõ đây là dữ liệu **tổng hợp**, không phải mô phỏng |
| RW0-3 | Bộ ghi tick thật: vòng đệm 30 giây các gói `simulation-tick`, bật bằng `?record=1`, xuất JSON, mở lại bằng `?replay=<file>` | file mới `utils/tickRecording.ts` | Ghi 10 giây từ app thật rồi phát lại trong trình duyệt thường. Đường xuất file xem D-4 |
| RW0-4 | Số đo khung hình cho tooling: `gl.info.render.calls` và `.triangles` theo từng lớp (địa hình, cây cỏ, sinh vật, nước) qua bề mặt diagnostics | `utils/diagnosticsWindow.ts`, `src/window-globals.d.ts` | Tổng khớp cách đo ở §2 trong ±1%; có bảng theo lớp ở ba tư thế chuẩn |
| RW0-5 | Chốt ngân sách §4 | tài liệu này | Bảng §4 mang số đo thay cho đề xuất |

### RW1 — Gói nhanh · không phụ thuộc RW2

| ID | Việc | Neo | Xong khi |
|---|---|---|---|
| RW1-1 | Mức thấp phải hạ độ phân giải thật | `WorldShowcase.tsx:117-122` (`QualityApplier`), `:399` (`dpr={[1, 1.5]}`) | Ở mức thấp, `canvas.width` bằng bề rộng CSS × 1. Hôm nay là × 1,5 |
| RW1-2 | Sàn ánh sáng ban đêm: thêm ánh trăng định hướng, nâng ambient ban đêm | `utils/skyParams.ts:24-32`, `WorldSky.tsx:149-161` | Test thuần: với mọi `t`, tổng cường độ sáng ≥ sàn (đề xuất 0,35). Tham số tại `t = 10` không đổi. Ảnh chụp `t = 0` thấy rõ đường bờ |
| RW1-3 | Ngày dài 10 phút mặc định, thêm nút "Luôn ban ngày" | `WorldShowcase.tsx:267` | Hàm thuần tính giờ có test; nút ghim `t = 11` |
| RW1-4 | 2D: mỗi con một hình, có màu | `PixiViewport.tsx:702-760` | Vitest draw-spy: con N đốt cho 1 hình đầu, N−1 đoạn thân, 1 thanh nước. Hai loại khác màu **và** khác hình |

Nguyên nhân của RW1-1 mới là suy đoán từ cách `@react-three/fiber` hoạt động: `<Canvas>` áp lại prop
`dpr` mỗi lần component cha render lại, mà `WorldShowcase` render lại mỗi 100 ms theo đồng hồ. Phải
xác nhận khi sửa.

### RW2 — Sinh vật khối ghép · thực hiện A1-4 và A1-5

| ID | Việc | Neo | Xong khi |
|---|---|---|---|
| RW2-1 | Hàm thuần dựng tư thế: gom đốt theo `agent_id`, đặt đốt gốc lên mặt đất, xếp đốt con theo vị trí tương đối × tỉ lệ sim→render, hướng từ `yaw/pitch/roll` theo `YXZ` | file mới `utils/agentPose.ts`; `utils/liveAgentTransform.ts` | Vitest: khứ hồi Euler với 3 bộ góc; con 3 đốt thẳng hàng cho 3 ma trận cách nhau đúng `length`. Trục dài của đốt được xác nhận bằng bản ghi thật |
| RW2-2 | Thân: hình trụ theo `length` và `radius`, cầu ở khớp, vẽ instanced. Bỏ `SEGMENT_RADIUS` | `LiveAgents.tsx:44,47,77` | Hai đốt khác `radius` cho hai ma trận khác nhau (DoD của A1-4). Số lệnh vẽ của sinh vật ≤ 5 với 10 con và với 1000 con |
| RW2-3 | Đầu: hai mắt theo `head_direction`; kẻ săn có mõm nhọn, con mồi đầu tròn | như trên | Ảnh chụp chuyển sang thang xám vẫn phân biệt được hai loại |
| RW2-4 | Màu: nền theo `agent_type`, lệch nhẹ theo `agent_id`, độ sáng theo năng lượng (giữ công thức hiện có) | `LiveAgents.tsx:149-157` | Cùng `agent_id` luôn cùng màu; hai loại không trùng màu |
| RW2-5 | Bóng tròn giả dưới đốt gốc khi bóng đổ thật tắt | mới | Thêm đúng 1 lệnh vẽ; ảnh mức Thấp cho thấy con vật đứng trên đất |
| RW2-6 | Nhìn từ xa: quá một khoảng cách, mỗi con là một dấu phẳng ≥ 6 px, hình và màu theo loại | mới | Ở tư thế `overview`, đếm trên ảnh ra đủ số con của fixture |
| RW2-7 | Chọn một con: vòng sáng và nhãn (loại, năng lượng, nước, số đốt) | mới; raycast `instanceId` → `agent_id` | Playwright: bấm vào một con của fixture, nhãn hiện đúng `agent_id` |
| RW2-8 | 2D vẽ theo cùng `length` và `radius` (A1-5) | `PixiViewport.tsx:679-700` | Vitest draw-spy |

Hai điều RW2-1 phải xác nhận, vì hiện mới là giả thuyết:

- Trục dài của đốt là Z cục bộ. Suy từ `mutate_genotype` kẹp `joint_anchor.z` theo `length / 2`
  (`src-tauri/src/evolution/mutation.rs`).
- `y` của mô phỏng là dải 0..10 không liên quan độ cao địa hình (ghi chú ở `LiveAgents.tsx:138-141`).
  Kế hoạch giả định hiệu `y` giữa các đốt của **cùng một con** vẫn có nghĩa.

Nhãn ở RW2-7 chỉ hiện thứ payload có. Payload không mang trường hành vi, nên "đang làm gì" phải chờ
`AgentCue` (N3) ở backend.

### RW3 — Mượt · thực hiện N1

| ID | Việc | Neo | Xong khi |
|---|---|---|---|
| RW3-1 | Giữ hai gói gần nhất kèm thời điểm nhận; vẽ trễ một chu kỳ phát (33 ms); nội suy vị trí, slerp hướng | `LiveAgents.tsx:117` (`useFrame`), `agentInterpolation.ts` | AN-S02 của [kế hoạch test Thế giới sống](../testing/2026-08-25-feature-living-world.md): cùng dãy gói, lấy mẫu ở 30/60/144 Hz cho cùng quỹ đạo trong sai số cho phép |
| RW3-2 | Gói trễ, thiếu, sai thứ tự; con mới sinh, con vừa chết; nhảy xa | | Không ngoại suy quá 1 chu kỳ. Con chết biến mất trong ≤ 1 chu kỳ. Nhảy xa hơn ngưỡng thì đặt thẳng, không trượt |
| RW3-3 | Không cấp phát trong `useFrame`: thêm biến thể ghi tại chỗ của `lerpVec3`; bảng tra `(agent_id, segment_id)` dựng một lần mỗi gói | `agentInterpolation.ts:22` | Không có `new` hay object literal trong vòng lặp khung |
| RW3-4 | Lớp làm mượt khớp tuỳ chọn (lọc tắt dần), sau một hằng số | | Chỉ làm nếu điểm dừng G-A kết luận chuyển động thật trông cứng |

Payload không mang số tick hay dấu thời gian, nên RW3-1 dùng thời điểm nhận ở giao diện.

### RW4 — Nhẹ

| ID | Việc | Neo | Xong khi |
|---|---|---|---|
| RW4-1 | Một bảng mức chất lượng duy nhất: `low`, `medium`, `high`, cùng chế độ `auto` | file mới `utils/qualityTiers.ts`; thay `type Quality` ở `WorldShowcase.tsx:75` | Mọi chỗ đọc mức chất lượng lấy từ bảng này; HUD có 4 lựa chọn |
| RW4-2 | Cây cỏ theo khoảng cách: chia ô; chỉ nạp ô gần và trong tầm nhìn vào bộ đệm instance; ô xa dùng hình thay thế ≤ 8 tam giác; rất xa thì bỏ | `WorldVegetation.tsx:69-79,150` | Đạt ngân sách §4 ở ba tư thế; số lệnh vẽ tăng không quá 2 |
| RW4-3 | Mật độ theo mức: Thấp 1/4, Vừa 1/2, Cao đủ | `WorldVegetation.tsx:75` | Số instance đúng tỉ lệ |
| RW4-4 | Chế độ capture giữ nguyên đường vẽ đầy đủ | `WorldShowcase.tsx:202-213`, `utils/captureMode.ts` | Chạy `npm run capture:views` trước và sau trên cùng một máy: 8 hash trùng nhau |
| RW4-5 | Tự chọn mức: đo thời gian khung khi cảnh đã ổn định rồi chọn mức. Khi đang chạy, nếu trung vị vượt ngân sách trong 2 giây thì hạ DPR, sau đó hạ mức. Chỉ nâng lại một lần, sau 10 giây dư. Lựa chọn tay luôn thắng | mới | Test thuần với dãy thời gian khung giả: hạ đúng lúc, không dao động qua lại |
| RW4-6 | Gate ngân sách: spec Playwright đọc số của RW0-4 ở ba tư thế × ba mức, đỏ khi vượt §4 | `tests/e2e/` (spec mới) | Đối chứng âm: nhân đôi mật độ cây thì spec đỏ và nêu tên lớp vượt |
| RW4-7 | Sinh vật theo khoảng cách: xa thì không nội suy đốt con, rất xa thì là dấu phẳng (RW2-6) | | Đạt ngân sách sinh vật §4 với fixture 1000 con × 15 đốt |

Số tam giác và lệnh vẽ không phụ thuộc GPU, nên RW4-6 chạy được trên CI. FPS thì không; nó thuộc
RW6. Khi PR #47 đã vào thân cây, spec mới phải được ghi vào danh sách spec của
`scripts/e2e_spec_policy.mjs` trong cùng commit.

Kế hoạch này **không** bật `TERRAIN_CHUNKED` (`WorldShowcase.tsx:65`). Địa hình chỉ chiếm 294.912 tam
giác, và ghi chú tại đó yêu cầu xem trên phần cứng thật trước khi bật mặc định.

### RW5 — Đẹp thêm · sau khi RW2–RW4 xong

| ID | Việc | Xong khi |
|---|---|---|
| RW5-1 | Thở và nhún trong vertex shader, pha lệch tất định theo `agent_id` | Không thêm lệnh vẽ; con đứng yên không đứng hình; cùng dãy gói cho cùng ảnh |
| RW5-2 | Sinh ra phóng to dần, chết mờ dần, mỗi thứ ≤ 300 ms | Chuỗi ảnh |
| RW5-3 | Màu theo dòng giống | Cần trường mới trong payload: xem D-2 |

### RW6 — Nghiệm thu trên máy thật

| ID | Việc | Xong khi |
|---|---|---|
| RW6-1 | Bảng máy × mức × FPS (trung vị và 1% thấp nhất) với fixture 10, 200 và 1000 con, kèm ảnh chụp | Có đủ ba máy: MacBook Air M5, máy case (RTX 5060 Ti) và Dell Vostro 3530 (GPU tích hợp). Ghi vào một tài liệu `docs/ai/testing/` |
| RW6-2 | Chụp lại 8 ảnh chuẩn nếu RW1 hoặc RW4 đổi thứ chúng nhìn thấy | `npm run capture:views` trên phần cứng thật; `tests/frontend/mapManifestEvidence.test.ts` xanh với ảnh mới. Không sửa tay `map_manifest.json` |
| RW6-3 | Map gate theo [`AGENTS.md`](../../../AGENTS.md): MCP `animal-map-vision`, ảnh before/after | Không tuyên bố phần bản đồ "xong" khi chưa qua gate này |

## 6. Thứ tự và điểm dừng

```text
RW0 ──► RW1                      (độc lập với RW2, làm song song được)
RW0 ──► RW2 ──► RW3 ──► [G-A] ──► RW4 ──► RW5 ──► RW6
        RW0-5 ──► [G-B]
```

| Điểm dừng | Câu hỏi | Nếu "không" |
|---|---|---|
| **G-A**, sau RW2 và RW3, xem bản ghi thật (RW0-3) | Chuyển động thật của các đốt có đủ mượt và dễ đọc không? | Làm RW3-4 trước RW4 |
| **G-B**, sau RW0-5 | Trần mức Thấp ở §4 có đạt được mà cảnh vẫn nhìn ra không? | Sửa trần hoặc bỏ thêm lớp ở mức Thấp, và ghi lại lý do |

Ước lượng công, chưa kiểm chứng: RW0 và RW1 mỗi gói khoảng 1 ngày; RW2 2–3 ngày; RW3 1–2 ngày;
RW4 3–4 ngày; RW5 2–3 ngày; RW6 1 ngày cộng thời gian đo trên máy của chủ dự án.

## 7. Gate cho mỗi PR

```bash
npm run lint && node scripts/eslint_ratchet.mjs
```

```bash
npm run test && npm run test:frontend && npm run build
```

```bash
npm run check:bundle && npm run check:docs
```

Thêm vào đó:

- E2E đủ bộ, kể cả `console_hygiene.spec.ts`: cảnh landscape không được in gì ra console.
- Tăng ngân sách bundle là một quyết định: đổi con số trong cùng commit gây ra mức tăng và nêu lý do.
- Job Frontend của CI đang đỏ ở `check:e2` trên thân cây (chờ QD-E2M). Cho tới khi việc đó được
  quyết, PR của kế hoạch này cũng sẽ hiện đỏ ở bước ấy.

## 8. Quyết định cần chủ dự án

| ID | Câu hỏi | Mặc định nếu không trả lời |
|---|---|---|
| D-1 | Máy yếu nhất cần hỗ trợ là máy nào, và 30 FPS ở mức Thấp có đủ không? | Dell Vostro 3530 là máy sàn; 30 FPS là đủ |
| D-2 | Màu theo dòng giống cần một trường mới trong `SegmentState`, tức là sửa `src-tauri/src`, binding ts-rs và [`IPC_CONTRACT.md`](../../reference/IPC_CONTRACT.md). Có làm không? | Chưa làm. Dùng màu theo loại và lệch theo `agent_id` |
| D-3 | Ngày dài 10 phút và nút "Luôn ban ngày" tắt sẵn có hợp không? | Như đề xuất |
| D-4 | Lấy bản ghi tick thật bằng đường nào? (a) Chủ dự án chạy `npm run tauri:dev` một lần trên máy case và ghi ở giao diện; luật hub cấm agent chạy app. (b) Tách đoạn dựng payload ở `simulation_loop.rs:2068` thành hàm công khai để một test headless xuất bản ghi tất định; đường này sửa `src-tauri/src` | (a) cho lần xem đầu. (b) khi có dòng uỷ quyền, vì chỉ (b) cho bản ghi lặp lại được trên CI |
| D-5 | Kế hoạch này dùng khối ghép làm cách vẽ mặc định, đẩy A2–A4 và N2–N4 của Thế giới sống ra sau. Có đồng ý thứ tự đó không? | Đồng ý; các nhánh kia giữ trạng thái `proposed` |

## 9. Rủi ro

| Rủi ro | Dấu hiệu | Cách xử lý |
|---|---|---|
| Chuyển động thật trông cứng hoặc rung | Thấy ở G-A | RW3-4; nếu vẫn chưa đạt thì xem lại ở backend, ngoài phạm vi kế hoạch này |
| Vị trí tương đối giữa các đốt không dựng được thân liền | Đốt rời nhau trong bản ghi thật | Dựng thân từ `joint_anchor_*` và cây cha-con thay cho toạ độ thế giới |
| Cây cỏ nháy khi đổi ô hoặc đổi hình thay thế | Thấy khi camera di chuyển | Vùng chuyển tiếp chồng lấn; chỉ dựng lại bộ đệm khi camera qua nửa ô |
| Bộ tự chọn mức dao động | Mức đổi qua lại | Luật "chỉ nâng một lần" ở RW4-5; test dãy thời gian khung |
| Ảnh chuẩn của bản đồ lệch với cảnh thật | Hash không đổi nhưng cảnh đã đổi | RW4-4 giữ đường vẽ capture; RW6-2 chụp lại khi cần |
| Bundle phình | `check:bundle` đỏ | Không thêm thư viện; hình học dựng bằng `three` đã có |
| Fixture tổng hợp bị hiểu nhầm là mô phỏng | Ảnh fixture xuất hiện như bằng chứng khoa học | Tên file, tham số URL và nhãn trên cảnh đều ghi "synthetic" |

## 10. Quan hệ với tài liệu khác

- **[Task backlog Thế giới sống](../implementation/2026-08-25-feature-living-world-tasks.md):** kế
  hoạch này thực hiện A1-4, A1-5 và N1-1 tới N1-3, giữ nguyên DoD của chúng. A1-1 tới A1-3 và phần
  hàm thuần của N1-1 đã có trong mã từ `970d963`. Kế hoạch này không sửa backlog đó; khi kế hoạch
  được chấp nhận thì cập nhật trạng thái các mục ấy ở đó.
- **[Thiết kế từ genome tới mesh](../design/2026-08-25-feature-genome-to-mesh.md):** §7 của thiết kế
  ấy giữ mesh procedural làm đường mặc định khi chưa có asset, và viết rằng một capsule đúng kích
  thước hữu ích hơn một khoảng trống. Khối ghép ở đây là bước đầu của đường đó. Nó đơn giản hơn
  compiler mesh của A2 (không xương, không skin) và không thay thế A2.
- **[Kế hoạch swarm](2026-11-05-swarm-master-plan.md):** map gate H10 vẫn là cổng của phần bản đồ
  (RW6-3). Các gói RW không chung file với thẻ `[RUST]` nào của Đợt 2 và Đợt 3.
- **[Hướng dẫn chạy coding agent](../../how-to/AGENT_WORKFLOW.md):** mỗi gói RW giao cho agent thì đi
  theo luật ở đó: worktree riêng, draft PR, verifier độc lập.
- **[Chia cho ba máy](2026-11-16-readable-world-three-machines.md):** lane, file sở hữu, thẻ RW-A1 … RW-C5 và
  ai kiểm bài của ai, cho ba agent `anima-mac`, `anima-case`, `anima-dell`.

## 11. Điều kế hoạch này không tuyên bố

- Không có số đo nào trên máy khác ngoài MacBook Air M5. Mục tiêu FPS cho máy yếu chưa được kiểm.
- Chưa ai nhìn thấy chuyển động thật của các đốt trên màn hình. Backend chưa được chạy cho việc này.
- Ngân sách ở §4 là đề xuất, chưa phải số đã đạt được.
- Bản đồ chưa được review theo quy trình `animal-map-vision`; MCP đó không kết nối trong phiên viết
  kế hoạch này.
- Chế độ capture (`?capture=1`) cho khung đen trong khung trình duyệt của phiên này; nguyên nhân chưa
  được tìm. Đường chính thức để chụp ảnh chuẩn vẫn là `npm run capture:views`.
- Đường xuất file của bộ ghi tick từ webview Tauri (RW0-3) chưa được thử.
