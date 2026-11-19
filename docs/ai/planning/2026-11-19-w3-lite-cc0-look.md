---
phase: planning
feature: real-world-and-bodies
title: Thẻ W3-lite — Giao diện bản đồ CC0, sửa lỗi shader/hình khối và 4 mục AMEND (prototype)
description: Vật liệu địa hình 6 lớp PBR CC0 + phân biệt toàn bộ 22 biome, cây cối CC0 low-poly kèm gió, vá 5 lỗi shader/lỗ trắng/viền cyan/thác chữ nhật/băng tầng, miệng hang đá rõ trên vách, màn nước thác kèm bọt mềm, và chú giải biome động trên minimap
status: completed
owner: rendering-frontend
last_reviewed: 2026-11-19
---

# Thẻ W3-lite — Diện mạo thế giới CC0 & 4 mục `AMEND W3-lite` (`prototype, to be replaced`)

> **Trạng thái nguyên mẫu (`prototype, to be replaced`):** mọi module mới (`assets/worldAssets.ts`, `materials/terrainPbrMaterial.ts`, `materials/floraWindMaterial.ts`) và phần mở rộng vật liệu/cảnh giữ nhãn nguyên mẫu để đánh giá trực quan và tốc độ khung hình trước khi thay bằng phiên bản sản phẩm hoàn chỉnh.

## 1. Vá 5 khuyết tật hình ảnh & hậu kỳ ACES / sương chân trời

- **Viền nước cyan (`WorldWater.tsx`):** bỏ viền phát sáng nhân tạo khi nước nông tiếp xúc đáy; giữ trong suốt theo độ sâu thật, ánh phản chiếu Fresnel theo hướng mặt trời và dải sương chân trời đồng bộ `scene.fog`.
- **Đốm biển (`WorldWater.tsx` + `worldTerrainTextures.ts`):** thay công thức nhiễu gây đốm sáng rải rác trên mặt biển xa bằng sóng đa tần mịn fades theo khoảng cách.
- **Thác nước hình chữ nhật & vệt sọc (`WorldWaterfalls.tsx`, gồm `AMEND W3-lite` mục 3):** thay `PlaneGeometry(1, 1)` phẳng bằng tấm thác thuôn hẹp phần mép trên (`0.62` ở đỉnh -> `1.04` ở chân) hơi cong parabolic khỏi vách đá, mép ngang khử bằng nhiễu hữu cơ (`if (alpha < 0.015) discard;`) để tuyệt đối không còn viền hình chữ nhật trắng, cùng shader bọt chân thác đồng tâm có làm mờ góc nhìn gần song song (`smoothstep(0.06, 0.34, abs(viewDir.y))`), giữ nguyên `2` draw calls (`waterfall-curtains` + `waterfall-foam`).
- **Dải sọc địa tầng & rìa tuyết cứng (`worldTerrainTextures.ts`):** bỏ hàm `sin` tạo dải sọc trên vách `Rock`/`Badlands`, thay bằng nhiễu hạt khoáng chất, đồng thời làm tơi rìa tuyết ở vùng dốc (`slope > 0.32`).
- **Hậu kỳ (`WorldShowcase.tsx` + `WorldSky.tsx`):** bật `ACESFilmicToneMapping` (`toneMappingExposure = 1.0`) và sương chân trời hòa vào nền trời.

## 2. Sáu lớp PBR địa hình CC0 + Phân biệt 22 biome (`AMEND W3-lite` mục 1)

- **Nguồn CC0 (`public/assets/world/textures/`, 4.19 MB tổng ảnh WebP):** tải 6 cặp `1K` diffuse/normal CC0 1.0 từ Poly Haven (`leafy_grass`, `forest_floor`, `cliff_side`, `coast_sand_01`, `snow_02`, `brown_mud_02`), chuyển WebP 1024×1024, khai báo đủ trong [`public/assets/world/SOURCES.json`](../../../public/assets/world/SOURCES.json) và đồng bộ [`NOTICE`](../../../NOTICE) qua `node scripts/gen_notice.mjs`.
- **Nạp luỹ tiến & fallback an toàn (`worldAssets.ts`, `terrainPbrMaterial.ts`):** nạp bất đồng bộ 6 lớp vào `DataArrayTexture` `RGBA8` (`512px` ở Low, `1024px` ở Medium/High) cùng xoay anti-tiling 2 lần lấy mẫu và triplanar mapping trên vách dốc; trả `null` khi lỗi/404 để giữ nguyên địa hình procedural.
- **Đọc rõ toàn bộ 22 biome (`worldTerrainTextures.ts` + `terrainPbrMaterial.ts`):**
  - Xuất `BIOME_DISPLAY_RGB` (22 màu phân biệt rõ trên cả `Desert`, `Savanna`, `Steppe`, `Chaparral`, `Badlands`, `Grassland`, `Shrubland`, `Forest`, `Jungle`, `Taiga`, `Tundra`, `Alpine`, `Swamp`, `Bog`, `Mangrove`, `Rock`, `Snow`, pale glacial `Glacier`), không làm đổi màu sinh vật theo biome.
  - Bảng độ nhám riêng theo từng biome (`BIOME_ROUGHNESS`, ví dụ `Glacier = 68`, `Bog = 118`, `Mangrove = 122`, `Swamp = 128`, `Jungle = 184`, `Snow = 192`, `Desert = 250`) và `buildSplatTexture` phủ trọng số `R/G/B/A` cho cả 22 biome.
  - `terrainPbrMaterial.ts` dùng micro-contrast PBR theo khoảng cách thay cho pha trộn 44% màu thô làm nhạt màu biome ở góc nhìn toàn cảnh.

## 3. Cây cối CC0 Low-Poly & Gió (`worldAssets.ts`, `floraWindMaterial.ts`, `WorldVegetation.tsx`)

- **Năm mẫu hình học CC0 siêu nhẹ (`public/assets/world/models/`, tổng 24.0 KB):**
  - `rock.glb` (`boulder_01`): `118` tris (`4,728` B)
  - `pine.glb` (`pine_sapling_small`): `141` tris (`5,160` B)
  - `tree_broad.glb` (`quiver_tree_01`): `206` tris (`7,316` B)
  - `shrub.glb` (`shrub_01`): `96` tris (`3,588` B)
  - `fern.glb` (`fern_02`): `74` tris (`3,208` B)
- Ánh xạ toàn bộ 14 `FloraType` giữ thân gỗ nâu tách khỏi tán lá xanh sáng, `DoubleSide` khi bật gió neo gốc (`floraWindMaterial.ts`), ghép hình học bằng `concatNonIndexedGeometries` nội bộ không kéo thêm `BufferGeometryUtils.js`, và tầng `Low` / 404 giữ fallback procedural.

## 4. Miệng hang đá (`caveGeometry.ts`, `WorldCaves.tsx`) & Chú giải Minimap (`WorldMinimap.tsx`)

- **Miệng hang rõ trên vách đá (`AMEND W3-lite` mục 2):** sửa lỗi toạ độ UV chưa canh giữa (`(cellX + size * 0.5) / size`) từng khiến hang bị kẹp xuống mực nước biển, tạo vòm hang (`mouthR = 1.65..2.50`, `depth = 1.9..3.0`) nằm gọn ngoài sườn dốc với vành đá phong hoá (`[0.23, 0.215, 0.2]`) ôm hốc tối đen (`[0.005, 0.005, 0.008]`) trong đúng `1` `BufferGeometry` (`1` draw call), tiếp tục gắn nhãn trang trí (`Hide/Show decoration` trên HUD).
- **Chú giải biome động trên minimap (`AMEND W3-lite` mục 4):** `WorldMinimap` quét `world.biome` / `riverAmt` / `water` một lần mỗi thế giới, mặc định mở panel `data-testid="minimap-biome-legend"` neo gọn bên trái canvas minimap (không che bản đồ, bật/tắt qua nút `legend`), hiển thị ô màu khớp `BIOME_DISPLAY_RGB` và tên chỉ cho các biome thực sự có mặt.

## 5. Kết quả đo `capture:map` (`RTX 5060 Ti`, Chrome 157, `?replay=synthetic&agents=200&segments=5`) & Các cổng

- **Theo 3 mức chất lượng (`overview` / `forest`):**
  - `low`: `1250` FPS med (`714.3` p5, `21` calls, `477,508` tris) / `1000` FPS med (`526.3` p5, `45` calls, `708,132` tris)
  - `medium`: `1111.1` FPS med (`666.7` p5, `21` calls, `558,022` tris) / `555.6` FPS med (`370.4` p5, `49` calls, `1,902,384` tris)
  - `high`: `666.7` FPS med (`384.6` p5, `27` calls, `726,260` tris) / `333.3` FPS med (`250` p5, `68` calls, `7,495,940` tris)
- **Đầy đủ 8 góc nhìn chuẩn `high` so với baseline `before-3f48f7e5` (`0` page errors, chỉ có `1` HTTP 404 vô hại của `favicon.ico`):** `overview 666.7 FPS`, `horizon 333.3 FPS`, `coast 370.4 FPS`, `peak 400 FPS`, `river 1666.7 FPS`, `forest 333.3 FPS`, `waterfall 333.3 FPS`, `creature 500 FPS`.
- **Ngân sách & kiểm thử:** `WorldShowcase` đạt `174.6 / 175 KiB` (tổng toàn bundle `1983.5 / 2000 KiB`), `check:flora-footprint` đạt với mọi đường kính cây khai báo, `gen_notice.mjs --check` khớp 100%, toàn bộ cổng `lint`, `typecheck:tests`, `test`, `test:frontend`, `check:ipc`, `check:csp`, `check:docs`, `check:audit` đều xanh.

## 6. Giới hạn đã biết

- Đánh số `capture:map` là góc nhìn tham chiếu trên dữ liệu synthetic trên `RTX 5060 Ti` (`Chrome` dev server), không đại diện cho `Intel Iris Xe` hay Tauri runtime.
- Hang vẫn là hốc lõm trang trí đặt trên lưới heightmap đơn lớp (chưa khoét hầm đi xuyên qua địa hình voxel/SDF).
