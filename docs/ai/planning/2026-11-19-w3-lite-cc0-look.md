---
phase: planning
feature: real-world-and-bodies
title: Thẻ W3-lite — Giao diện bản đồ CC0, sửa lỗi shader/hình khối và 4 mục AMEND (prototype)
description: Vật liệu địa hình 6 lớp PBR CC0 + phân biệt toàn bộ 22 biome, cây cối CC0 low-poly rút gọn theo hình dáng, vá 5 lỗi shader/viền cyan/dải cam/thác/băng tầng, miệng hang rõ, chú giải minimap, giải phóng bộ nhớ N5 và đạt ngân sách §4a ở cả High/Medium/Low
status: completed
owner: rendering-frontend
last_reviewed: 2026-11-19
---

# Thẻ W3-lite — Diện mạo thế giới CC0 & Vòng sửa 1 (`prototype, to be replaced`)

> **Trạng thái nguyên mẫu (`prototype, to be replaced`):** mọi module mới (`assets/worldAssets.ts`, `materials/terrainPbrMaterial.ts`, `materials/floraWindMaterial.ts`) và phần mở rộng vật liệu/cảnh giữ nhãn nguyên mẫu để đánh giá trực quan và tốc độ khung hình trước khi thay bằng phiên bản sản phẩm hoàn chỉnh.
> **Ghi nhận ảnh chụp chuẩn (`?capture=1`):** `WorldTerrain`, `WorldTerrainLod` và `WorldVegetation` bỏ qua việc nạp PBR/GLB bất đồng bộ khi `?capture=1` để giữ `npm run capture:views` xác định từng byte.

## 1. Vá khuyết tật hình ảnh & hậu kỳ (`WorldShowcase.tsx`, `WorldWater.tsx`, `WorldSky.tsx`, `WorldWeather.tsx`, `worldTerrainTextures.ts`)

- **Viền nước cyan & lỗ nền trời dưới sông băng (`WorldWater.tsx`, `oceanShelf.ts`):** giữ `OCEAN_SHELF_BAND_FRACTION = 0.1` (`120` đơn vị ở thế giới `1200`), tăng độ đục đại dương sâu (`pow(depthN, 0.78)` + `oceanDeepOpaque`) để đáy biển được chiếu sáng bên trong lưới không lộ qua nước sâu, và hoà mép ngoài (`boxN + nEdge` từ `0.84..0.98` kết hợp `edgeHard` ở `-4%..-1%`) thành nước sâu đục `100%` trước khi chạm ranh giới vuông — loại bỏ hoàn toàn viền sáng cyan cũng như khe trời dưới vách băng phía nam.
- **Đốm biển (`WorldWater.tsx`):** nhiễu vi mô tắt hẳn trước cự ly toàn cảnh (`0.48 * terrainSize`), giảm bậc phản chiếu xuống `pow(ndh, 28.0) * 0.42 * detail^2`.
- **Thác nước hình chữ nhật & vệt sọc (`WorldWaterfalls.tsx`, `AMEND #3`):** tấm thác thuôn hẹp phần đỉnh (`0.62 -> 1.04`) uốn cong nhẹ, khử mép nhiễu (`if (alpha < 0.015) discard;`), cùng shader bọt chân thác đồng tâm có làm mờ góc nhìn gần song song (`2` draw calls).
- **Dải sọc địa tầng, dải cam dãy núi bắc & rìa tuyết cứng (`worldTerrainTextures.ts`, `terrainPbrMaterial.ts`):**
  - Bỏ hàm `sin` tạo dải sọc địa tầng; làm dịu màu `Chaparral` (`[146, 136, 88]`) và `Badlands` (`[168, 116, 82]`), thêm nhiễu uốn miền (`smoothNoise2D`) chuyển tiếp ranh giới cho `Badlands`/`Chaparral`/`Snow`/`Rock`/`Alpine` + vân địa chất loang tự nhiên cho `Badlands`.
  - Giảm 72% độ bão hoà sắc cam từ ảnh `cliff_side` trên vách đá dốc (`wR`) trong `terrainPbrMaterial.ts`, giữ nguyên tương phản độ sáng và chi tiết normal triplanar.
- **Hậu kỳ & danh sách dòng `WorldShowcase.tsx` (sửa mục N3):** R3F v8 đã mặc định dùng `ACESFilmicToneMapping` khi không truyền `flat`; thẻ W3-lite đặt rõ `toneMappingExposure: 1.04` và truyền `tier={effectiveQuality}` qua đúng **4 dòng** trong `src/components/Landscape/WorldShowcase.tsx`:
  - `:475` `toneMapping: THREE.ACESFilmicToneMapping,`
  - `:476` `toneMappingExposure: 1.04,`
  - `:550` `tier={effectiveQuality}` (`WorldTerrainLod`)
  - `:558` `tier={effectiveQuality}` (`WorldTerrain`)

## 2. Sáu lớp PBR địa hình CC0, Phân biệt 22 biome & Giải phóng VRAM (`N5`)

- **Nguồn CC0 (`public/assets/world/textures/`, `4,051,418` B = `4.05 MB` / `3.86 MiB`):** 6 cặp WebP `1024×1024` (dự phòng thay thế KTX2 theo thẻ) CC0 1.0 từ Poly Haven (`leafy_grass`, `forest_floor`, `cliff_side`, `coast_sand_01`, `snow_02`, `brown_mud_02`), khai báo đầy đủ URL/bản quyền trong [`public/assets/world/SOURCES.json`](../../../public/assets/world/SOURCES.json) và đồng bộ [`NOTICE`](../../../NOTICE).
- **Nạp luỹ tiến, tách chunk & giải phóng tầng High (`worldAssets.ts`, `N5`, `N6`):** nạp bất đồng bộ 6 lớp vào `DataArrayTexture` `RGBA8` (`512px` ở Low, `1024px` ở Medium/High) qua dynamic `import('./assets/worldAssets')`. Khi `loadTerrainPbrArrays('low')` chạy lúc Auto hạ tầng (hoặc đang tải dở), `disposeOtherTerrainPbrTiers('low')` tự động huỷ ngay hai `DataArrayTexture` `1024²` và huỷ kết quả đang chờ để không rò rỉ GPU/JS memory.
- **Đọc rõ 22 biome (`worldTerrainTextures.ts`):** `BIOME_DISPLAY_RGB` và `BIOME_ROUGHNESS` phủ đủ 22 biome không làm đổi màu sinh vật.

## 3. Cây cối CC0 Low-Poly rút gọn theo hình dáng & Ngân sách §4a (`worldAssets.ts`, `floraWindMaterial.ts`, `WorldVegetation.tsx`)

- **Năm mẫu GLB CC0 rút gọn qua `@gltf-transform/cli` `weld` + `simplify` (`public/assets/world/models/`, tổng `15,912` B):**
  - `pine.glb` (`pine_sapling_small`): `64` tris (`3,248` B)
  - `tree_broad.glb` (`quiver_tree_01`): `90` tris (`4,196` B)
  - `shrub.glb` (`shrub_01`): `52` tris (`2,508` B)
  - `rock.glb` (`boulder_01`): `90` tris (`3,656` B)
  - `fern.glb` (`fern_02`): `38` tris (`2,304` B)
- **Tổng tài sản dưới `public/assets/world/`:** `4,076,855` B (`4,051,418` B ảnh + `15,912` B mô hình + `9,525` B `SOURCES.json`, `3.89 MiB <= 20 MB`). Các GLB là hình khối dẫn xuất rút gọn từ lưới gốc Poly Haven (chưa có script tái tạo tự động trong repo, đã ghi rõ trong `SOURCES.json` theo mục N8).
- **Ánh xạ đúng hình dáng (`FLORA_STYLE_MAP`):** áp dụng GLB cho 6 loài khớp dáng (`Pine`, `Round`, `Jungle`, `Bush`, `Rock`, `Reed`), giữ `Cactus`, `Acacia`, `Palm`, `DeadTree`, `Tuft`, `Coral`, `Kelp`, `Seagrass` trên hình học procedural nhẹ vốn có để đúng hình dáng và giữ dư địa lớn dưới ngưỡng §4a ở mức `High`.
- **Fallback xác thực qua `fetch` + `GLTFLoader.parseAsync` (`N2`):** đã kiểm tra cả nạp GLB thật thành công lẫn lỗi 404 trả về `null` (và kiểm tra thử nghiệm phá fallback khiến test đỏ với `Flora GLB missing: pine (404)`).

## 4. Miệng hang đá (`caveGeometry.ts`, `WorldCaves.tsx`) & Chú giải Minimap (`WorldMinimap.tsx`)

- **Miệng hang (`AMEND #2`):** sửa UV canh giữa `(cellX + world.size * 0.5) / world.size`, tạo vòm hang tối (`mouthR = 1.65..2.50`, `depth = 1.9..3.0`) trong 1 draw call trang trí.
- **Minimap (`AMEND #4`):** chú giải `data-testid="minimap-biome-legend"` mặc định mở cạnh canvas 192×192, chỉ liệt kê biome hiện diện trong `world`.

## 5. Kết quả đo `rw_budget.spec.ts`, `capture:map` (`RTX 5060 Ti`) & Ngân sách bundle

- **Cổng ngân sách §4a (`tests/e2e/rw_budget.spec.ts` — 4/4 bài đạt, không sửa ngưỡng §4a):**
  - `low`: `overview` `471,932` tris / `17` calls (flora `76,134` / `1`), `ecosystem` `636,856` tris / `29` calls (flora `241,058` / `13`), `spawn` `556,136` tris / `29` calls (flora `160,338` / `13`).
  - `medium`: `overview` `548,132` / `17` (flora `152,334` / `1`), `ecosystem` `940,304` / `31` (flora `544,506` / `15`), `spawn` `790,282` / `29` (flora `394,484` / `13`).
  - `high`: `overview` `711,764` / `21` (flora `309,378` / `1`), `ecosystem` `2,619,224` / `42` (flora `2,216,838` / `22`, đạt `44.1%` trần `5,030,000` flora và `43.7%` trần `6,000,000` tổng), `spawn` `2,102,494` / `40` (flora `1,700,108` / `20`).
  - Đối chứng âm (`x4`): báo đỏ đúng lớp `flora` (`[total] triangles 1,360,128 > 1,200,000`, `[flora] triangles 964,330 > 530,000` tại `low/ecosystem`).
- **Đo trực quan `capture:map` trên `RTX 5060 Ti` (`Chrome 157` dev server — không đại diện cho `Intel Iris Xe` hay Tauri runtime):**
  - **8 góc `High` (`--agents 200`, `0` page errors):** `overview` `454.5` FPS (`27` calls, `726,260` tris), `horizon` `344.8` FPS (`74` calls, `3,346,182` tris), `coast` `400` FPS (`68` calls, `3,283,300` tris), `peak` `476.2` FPS (`63` calls, `3,082,066` tris), `river` `1111.1` FPS (`45` calls, `2,298,918` tris), `forest` `416.7` FPS (`68` calls, `3,506,496` tris — giảm từ `7,495,940`), `waterfall` `400` FPS (`67` calls, `3,681,986` tris), `creature` `666.7` FPS (`41` calls, `661,662` tris).
  - **4 góc `Low` (`--agents 1000`, `0` page errors):** `overview` `333.3` FPS (`21` calls, `479,108` tris), `forest` `250` FPS (`45` calls, `733,052` tris), `coast` `188.7` FPS (`44` calls, `659,724` tris), `creature` `263.2` FPS (`40` calls, `612,788` tris).
- **Bundle & Lưu ý thẻ tiếp theo (`N6`):** tách dynamic import `worldAssets.ts` (`6.87 kB` lazy chunk) đưa `WorldShowcase` về **`170.8 / 175 KiB` (`98%`, còn `4.2 KiB`)**, nhưng tổng JS toàn app đã ở mức **`1991.3 / 2000 KiB` (`99.6%`, chỉ còn `8.7 KiB`)** sau khi gộp trunk (`W2-lite`), nên thẻ tiếp theo chạm vào bundle sẽ cần quyết định ngân sách tổng hoặc tách nhỏ thêm.

## 6. Giới hạn đã biết (theo dõi ở các thẻ sau)

- Đo lường `capture:map` chỉ thực hiện trên `RTX 5060 Ti`; đánh giá chi phí shader mức `Low` trên `Iris Xe` thuộc thẻ `C4` (`N4`).
- Mật độ `WorldFish` vùng ven bờ và việc đồng bộ hình dạng `sim-env-trees` trong `simEnvironment.ts` nằm ngoài phạm vi W3-lite.
- Hang vẫn là hốc lõm trang trí trên lưới heightmap đơn lớp (chưa khoét hầm voxel/SDF).
