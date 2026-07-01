# Danh sách công việc & Kế hoạch tiếp theo (TODO)

> ⬇️ Mục đang làm: **World Terrain Overhaul** (ở ngay dưới). Phần "Mô hình Thỏ Papercraft" cũ được giữ lại ở cuối file như lưu trữ.

---

# 🌍 [ĐANG LÀM] World Terrain Overhaul — Lục địa khổng lồ + Biome + Cache

**Cập nhật:** 2026-07-01. **Trạng thái:** `npm run build` ✅, `npm run test:frontend` ✅ 237/237, lint 0 lỗi. Các bước 1→5 **ĐÃ COMMIT**; bản nâng cấp "to/rộng/nhiều biome" ở mục 🚀 bên dưới.

## Bối cảnh
Đập đi xây lại phần sinh + render địa hình cho `landscape.html`: lục địa khổng lồ (data 1024²), noise sắc nét (ridged + domain warp), biome Whittaker (nhiệt×ẩm), và cache nhị phân để load tức thì từ lần 2.

## Kiến trúc (đã chạy thật)
- **Dữ liệu SoA TypedArrays** (KHÔNG còn `TerrainCell[][]`): `World` gồm `elevation/moisture/temperature/flow: Float32Array`, `biome: Uint8Array`, flora SoA. ~17MB @1024².
- **Cache nhị phân**: IndexedDB structured-clone (binary, không JSON) → lần 2 đọc thẳng buffer vào RAM, bỏ qua thuật toán. Key có `WORLD_GEN_VERSION` để invalidate.
- **Tách data-resolution khỏi mesh-resolution**: data 1024² nhưng mesh render ~256 segment sample heightmap → chi tiết cao, không cần 1M vertex.

## Các file chính (đều mới, dưới `src/components/Landscape/`)
| File | Vai trò |
|---|---|
| `utils/worldGen.ts` | Bộ sinh SoA: fBm 8-octave + ridged + domain warp + D8 flow (sông) + temperature(vĩ độ+lapse) + moisture(2 tầng + bốc hơi) + Whittaker 14 biome + flora SoA. Sinh 1024² ~1s, deterministic. |
| `utils/worldCache.ts` | memo + IndexedDB binary + `loadOrGenerateWorld()` / `getMemoizedWorld()` / `clearWorldCache()`. |
| `utils/worldGen.worker.ts` | **(Bước 1 vừa xong)** Web Worker sinh map off-thread, transfer zero-copy buffer về. |
| `WorldTerrain.tsx` | Mesh biome-color, displaced, winding CCW (normal +Y). Props: `renderSize`(400), `heightRatio`(0.13), `meshResolution`(256). |
| `WorldVegetation.tsx` | Cây instanced từ flora SoA (thông/tán tròn/jungle/xương rồng/đá theo biome). |
| `WorldShowcase.tsx` | Canvas + async-load world (cache) + terrain + vegetation + ocean plane + OrbitControls + fog/light. Hằng: `WORLD_SIZE=1024`, `RENDER_SIZE=400`, `HEIGHT_RATIO=0.13`, `MESH_RES=256`. |
| `src/landscape.tsx` | Đã trỏ entry `landscape.html` → `WorldShowcase` (component cũ `LandscapeShowcase` vẫn còn cho test). |

## ✅ Đã hoàn thành
- [x] Bộ sinh SoA huge-scale + noise sắc nét + Whittaker 14 biome (verify @1024²: ~1s, 0 NaN, đủ biome).
- [x] Cache nhị phân IndexedDB (versioned).
- [x] Render 3D: WorldTerrain (mesh-decoupled) + WorldVegetation (instanced) + ocean + orbit + fog.
- [x] Wire vào `landscape.html`.
- [x] **BƯỚC 1 — Web Worker**: sinh 1024² off-thread, **không treo UI**; có fallback sync (test/SSR). Vite bundle thành chunk riêng (`worldGen.worker-*.js`). Verify: render giống hệt, build+test xanh.

## ✅ BƯỚC 2 — Nước đẹp hơn (Water shader) — XONG (2026-07-01)
File mới `src/components/Landscape/WorldWater.tsx` thay `ocean plane` phẳng cũ trong `WorldShowcase.tsx`.
- [x] **Custom GLSL ShaderMaterial** cho mặt biển: sóng swell động theo time (vertex displace), màu theo **độ sâu** (sample heightmap `DataTexture` Float32 → đáy: nông teal `#3fcfe0`, sâu xanh đậm `#06203f`).
- [x] **Bọt bờ biển (foam)**: dải trắng fbm-noise nơi `depth → 0`, fade mềm đúng waterline (không còn cạnh plane cứng).
- [x] **Sông**: quads cho cell `Biome.River` (từ `world.flow`), nâng nhẹ trên terrain, ripple chảy nhanh. (Hồ tách riêng — xem Bước 5.)
- [x] Fresnel sky-tint + sun specular (uniform `uSunDir` = hướng tới mặt trời, khớp directional light scene).
- [x] `fog={false}` trên cả 2 ShaderMaterial (tránh crash `refreshFogUniforms`).
- **Verify:** `npm run build` ✅, `npm run test:frontend` ✅ 237/237, lint 0 lỗi. Heightmap tex 1024² Float32 ~4MB GPU.
- **Tinh chỉnh:** màu nông/sâu → `uShallow`/`uDeep`; độ trong → `uOpacity`; biên độ sóng → `uWaveAmp`; bề rộng ribbon sông → `half` trong `riverGeom`.

## ✅ BƯỚC 3 — Bầu trời + ngày/đêm + thời tiết — XONG (2026-07-01)
2 file mới + wire vào `WorldShowcase.tsx`. Scale lớn cho world mới (terrain ±200, camera tới 1200, far 4000) — KHÔNG dùng lại Sky/Weather cũ vì chúng tuned cho world ~±100 (FogExp2 0.005 sẽ trắng xoá map 400).
- `WorldSky.tsx`: dome BackSide R≈2600 (`fog={false}` để giữ gradient trời), mặt trời/mặt trăng quỹ đạo R≈1800 + directional light (shadow), hemisphere + ambient theo `getSkyParams(timeOfDay)`, sao (700), mây trôi (8 cụm, scale ×4). Tự set `scene.background` theo skyColor mỗi frame. Export `sunDirectionForTime(t)` → dùng chung cho water specular.
- `WorldWeather.tsx`: mưa/tuyết (particle box ±RENDER_SIZE) + **linear `THREE.Fog`** near/far theo `worldScale` (clear: 800→3200; rain/snow/fog dày dần), màu fog đổi theo weather + ngày/đêm, ease mượt khi chuyển. Sở hữu `scene.fog`.
- `WorldShowcase`: state `timeOfDay`/`speed`/`weather`, đồng hồ ngày/đêm auto (`setInterval`, pause khi speed=0), HUD nhỏ (clock + play/pause + speed 0.5/1/2/4× + 4 nút weather), truyền `sunDir` động vào `WorldWater` (specular bám mặt trời, cập nhật mỗi frame trong useFrame). Bật `shadows` trên Canvas. Bỏ ambient/directional/fog tĩnh cũ.
- **Verify:** `npm run build` ✅, `npm run test:frontend` ✅ 237/237, lint 0 lỗi.
- **Tinh chỉnh:** tốc độ ngày/đêm → bước `0.05*speed` trong interval; độ dày fog → `fogProfile()` trong WorldWeather; bán kính dome/sao → hằng trong WorldSky.

## ✅ BƯỚC 4 — Minimap + HUD đầy đủ — XONG (2026-07-01)
1 file mới + nâng cấp HUD + cầu nối camera↔overlay trong `WorldShowcase.tsx`.
- `WorldMinimap.tsx`: bản đồ top-down vẽ `world.biome` qua `BIOME_RGB` + hillshade nhẹ theo elevation (pre-render 192² ImageData 1 lần/world). Marker mục tiêu (đỏ) + mũi tên hướng nhìn, cập nhật qua rAF (không re-render React mỗi frame). **Click để teleport** camera. Toggle **legend** 10 biome. Export type `CameraView`.
- **Cầu nối camera↔HTML overlay:** không dùng `window.activeCamera` global như Minimap cũ — thay bằng `viewRef` (mutable ref) mà `OrbitCam` ghi vào mỗi frame trong `useFrame` (targetX/Z, camX/Z); minimap & HUD đọc ref. `teleportRef` chứa lệnh teleport đang chờ → OrbitCam dời cả `controls.target` lẫn camera (giữ nguyên góc nhìn).
- **HUD nâng cấp:** đồng hồ + **phase** (🌙/🌅/☀/🌇), Play/Pause, speed 0.5–4×, 4 nút weather, **toạ độ 📍 x/z** (đọc từ viewRef qua interval 300ms, không re-render mỗi frame), nút **⟲ Reset** view (teleport về gốc).
- **Refactor:** `sunDirectionForTime()` chuyển sang `utils/skyParams.ts` (dùng chung sky+water, tránh warning fast-refresh).
- **Verify:** `npm run build` ✅, `npm run test:frontend` ✅ 237/237, lint **0 lỗi** (429 warning cũ, không phát sinh mới).

## ✅ BƯỚC 5 — Hydraulic erosion + hồ nước — XONG (2026-07-01)
Thay đổi ở tầng generation (`worldGen.ts`) + render (`WorldWater.tsx`, `WorldMinimap.tsx`). **Bump `WORLD_GEN_VERSION` 1→2** (cache cũ tự invalidate); worker transfer thêm `water.buffer`.
- **Erosion (droplet):** `hydraulicErosion()` chạy Pass 1b (sau elevation, TRƯỚC flow+biome nên sông/biome bám lòng đã khắc). Mô phỏng hạt mưa cuốn/lắng phù sa (inertia/capacity/erode/deposit/evaporate/gravity, 30 bước/hạt), phân bổ trên 4 góc bilinear. Deterministic (rng riêng seed từ baseSeed). Số hạt scale theo diện tích: `min(120k, n*0.06)`. Clamp [0,1] sau khi khắc.
- **Hồ (priority-flood, Barnes et al.):** `computeLakes()` Pass 4b — flood từ biển/rìa map vào trong bằng min-heap (typed-array), nâng mỗi cell tới ngưỡng tràn (sill). Nơi mặt filled > đất thật → hồ; lưu `water: Float32Array` (cao độ mặt nước, 0 nếu không có hồ). `LAKE_MIN_DEPTH=0.006` lọc vũng nông (speckle từ erosion). Cây không mọc trên ô hồ.
- **Render:** `WorldWater` thêm mesh `world-lakes` — quads phẳng tại `water*heightUnits`, shader `uWaterType=2` (dùng nhánh depth-color + foam như đại dương, khác nhánh river). `WorldMinimap` tint ô hồ sang xanh nước.
- **Verify:** `npm run build` ✅, `npm run test:frontend` ✅ 237/237, lint 0 lỗi. Smoke `generateWorld(256²)`: 115ms, elev∈[0,1], 735 ô hồ, 0 NaN. (1024² ước tính ~1.5–2.5s off-thread, cache sau lần đầu.)
- **Tinh chỉnh:** độ mạnh khắc → `erosionDroplets` / các hằng trong `hydraulicErosion`; ngưỡng hồ → `LAKE_MIN_DEPTH`; màu hồ → `lakeUniforms` (`uShallow`/`uDeep`); tắt hẳn → opts `erosion:false` / `lakes:false`.

> 🎉 **World Terrain Overhaul HOÀN TẤT các bước 1→5.** Còn tùy chọn tương lai: thermal erosion, hồ có sông nối (inlet/outlet), phản chiếu mặt nước (reflection probe).

---

# 🚀 NÂNG CẤP MAP — To hơn, Rộng hơn, Nhiều môi trường nhất (2026-07-01)

Mục tiêu người dùng: "map to, rộng, nhiều môi trường nhất". **Bump `WORLD_GEN_VERSION` 2→3** (cache tự sinh lại).

## To hơn (data) & Rộng hơn (world-space)
- `WORLD_SIZE` **1024 → 2048** (~4M cell, gấp 4× chi tiết). Sinh 1 lần off-thread **~5s @2048²** rồi cache; RAM thường trú ~90MB.
- `RENDER_SIZE` **400 → 1000** (rộng gấp 2.5×), `HEIGHT_RATIO` 0.13→0.14, `MESH_RES` 256→**384** (mesh chi tiết hơn).
- Camera: `near=2`, `far=RENDER_SIZE*11` (phải > dome `worldScale*6.5`); orbit `min=RENDER_SIZE*0.06`, `max=RENDER_SIZE*3.2` (< dome để camera luôn trong vòm trời). Sky/fog/dome tự scale theo `worldScale=RENDER_SIZE`. `maxFlora` 60k→**90k**.

## Nhiều môi trường (biome) — 14 → **22**
Thêm 8 biome: **Lake, Mangrove, Chaparral, Steppe, Alpine, Badlands, Glacier, Bog** (enum + `BIOME_RGB` + flora + legend minimap đủ 22 mục).
- `classify()` viết lại theo **dải elevation × Whittaker (nhiệt×ẩm)**: bờ nóng-ẩm→Mangrove; đỉnh→Glacier/Snow/Rock; dải alpine→Alpine/Rock; trũng ẩm→Bog(lạnh)/Swamp(ấm); nóng: Desert→Badlands→Savanna→Chaparral→Jungle; ấm/ôn: Steppe→Grassland→Shrubland/Forest; lạnh: Tundra/Taiga.
- **Tinh chỉnh khí hậu để mọi biome xuất hiện thật:** nhiệt độ trải rộng hơn (`lat*0.78 + tNoise*0.22 - lapse`), bay hơi `(temp-0.5)*0.5`, ẩm nền `mBase*0.95` → dải ẩm rộng đủ chứa cả Desert (khô kiệt) lẫn Jungle (sũng nước).
- Ô hồ được recolor `Biome.Lake`; minimap tint xanh; cây không mọc trên hồ.

## Verify (đo thật bằng smoke test tạm, đã xoá)
- **@2048² (độ phân giải thật): present = 22/22 biome**, 5.0s, 49.3k ô hồ, 0 NaN. Phân bố lành mạnh (Grassland/Shrubland/Forest/Taiga/Chaparral nhiều; Desert/Steppe/Tundra/Glacier hiếm nhưng có).
- @512²: 22/22, 0.36s. `npm run build` ✅ · `npm run test:frontend` ✅ 237/237 · lint 0 lỗi.
- **Tinh chỉnh nhanh:** kích thước data → `WORLD_SIZE`; độ rộng → `RENDER_SIZE`; ngưỡng biome → `classify()`; độ khô sa mạc → hệ số `evaporation` + ngưỡng `Desert/Badlands`; tắt biome nước → opts `lakes:false`.

---

# 🛠 FIX RENDER — Sông/Hồ/Đá bay (2026-07-01)

Sửa triệt để 3 lỗi hiển thị. **Bump `WORLD_GEN_VERSION` 3→4** (cache sinh lại). File mới `utils/worldSample.ts` (sampler dùng chung).

1. **Sông/suối** — BỎ hoàn toàn quad rời rạc trong `WorldWater`. Giờ **bake vào vertex color của terrain** (`WorldTerrain`): sample `flow` bilinear → trộn màu nước xanh liên tục theo dòng chảy + **khoét rãnh nông** (`e -= riverAmt*0.02`) để sông nằm trong lòng. Hòa vào địa hình, không còn hình vuông đứt đoạn.
2. **Hồ nước** — thay vì ghép nhiều ô vuông: `computeLakes()` giờ gom **connected-component** thành từng bồn (lọc bồn nhỏ, giữ ≤280 bồn lớn nhất), trả về `lakeBasins[{level,bbox}]`. `WorldWater` sinh **MỘT plane/bồn** phủ bbox tại `level`; shader fade alpha→0 nơi cạn nên plane chỉ hiện trên phần chìm (không tràn ra đất). Thêm **viền cát**: `computeShore()` (BFS khoảng cách tới nước) → `WorldTerrain` trộn màu cát ẩm sát mép nước (biển + hồ).
3. **Đá bay** — nguyên nhân: flora lấy Y từ elevation full-res 2048² nhưng terrain render ở mesh 384 → lệch trên núi. Fix: `sampleMeshHeight()` lấy đúng cao độ **mặt mesh** (bilinear trên lưới mesh), và seat mỗi geometry bằng `boundingBox.min.y` để đáy chạm đất. Đá/cây bám sát mặt đất tuyệt đối.
- **Verify:** `npm run build` ✅ · `npm run test:frontend` ✅ 237/237 · lint 0 lỗi. Smoke @1024²: 95 bồn hồ, shore band 120k ô, mesh-snap khớp tuyệt đối (sai số <1e-5), 0 NaN. @2048² bồn hồ ≤280 planes.

---

# 🌱 NÂNG CẤP BIOME — Slope map + màu tương phản + foliage theo hệ sinh thái (2026-07-01)

Địa hình trước đây "đơn điệu" vì thiếu **độ dốc (slope)** (núi vẫn xanh cây như đồng bằng) và màu biome hơi nhạt. **Bump `WORLD_GEN_VERSION` 4→5.**
- **Slope map (`computeSlope`)** — signal thứ 3 (ngoài height+moisture): gradient elevation đo trên stencil rộng (`step≈size/200`) để phản ánh sườn núi lớn, không phải nhiễu erosion; chuẩn hoá về [0,1]. Lưu `slope: Float32Array`.
- **Bãi cát**: dải Beach rộng hơn (`seaLevel+0.022`) + viền cát `shore` — màu cát, **không cây**.
- **Biome theo Height×Moisture×Temp×Slope**: giữ 22 biome (Desert khô/thấp, Swamp ẩm/thấp, Forest·Jungle ẩm/trung, Grassland·Steppe trung, Snow/Glacier cực cao). **Thêm vách đá**: `slope>0.85` → `Rock` (chỉ ~3% đất — sườn dốc nhất), phần đất thoải giữ nguyên biome xanh. Thung lũng = vùng thoải+ẩm (flow tụ) → Forest/Swamp.
- **Màu tương phản hơn** (`BIOME_RGB`): sa mạc vàng đậm, rừng xanh tươi, cỏ xanh sáng, tuyết trắng tinh, badlands nâu đỏ…
- **Foliage theo hệ sinh thái**: `floraForBiome` + `floraDensity` theo biome; **mật độ ×(0.5+moisture)** (rừng ẩm dày, đồng khô thưa); **không cây trên slope>0.78** (vách đá) và trên nước/bãi cát.
- **Render**: `WorldTerrain` trộn **màu đá xám theo slope** (`smoothstep(0.55,1.0)`) → sườn dốc lộ đá, phá thế xanh đơn điệu; giữ blend cát + sông.
- **Verify:** build ✅ · 237/237 ✅ · lint 0 lỗi. Smoke @1024²: 22/22 biome, slope>0.85 = 3.2% đất (không phải cả bản đồ hoá đá), 0 cây trên vách dốc, 0 NaN.
- **Tinh chỉnh:** độ nhạy slope → hệ số `0.06` + `step` trong `computeSlope`; ngưỡng vách đá → `slope>0.85` trong `classify`; độ đậm tint đá → `smoothstep` trong WorldTerrain; mật độ cây → `floraDensity` × `(0.5+moisture)`.

## Cách chạy / kiểm tra nhanh
- Dev: `npm run dev` → mở `http://localhost:5173/landscape.html` (lần đầu sinh ~1s off-thread, sau đó cache → tức thì).
- Sinh thế giới mới: gọi `clearWorldCache()` hoặc bump `WORLD_GEN_VERSION` trong `worldGen.ts`.
- Tinh chỉnh: núi cao/thấp → `HEIGHT_RATIO`; to/nhỏ → `RENDER_SIZE`; chi tiết mesh → `MESH_RES`; nhiều desert hơn → ngưỡng trong `classify()` của `worldGen.ts`.
- Ảnh preview tĩnh (biome+hillshade) đã sinh: `world_preview.png` ở thư mục gốc.

## Lưu ý
- **Chưa commit** toàn bộ (world system + các fix landscape trước đó). Cân nhắc gom commit logic.
- `landscape.html` giờ hiển thị World mới; `LandscapeShowcase` (cũ) vẫn được 237 test dùng — **đừng xóa**.

---

# 🐰 [LƯU TRỮ] Mô hình Thỏ 3D Papercraft (task cũ)

Hệ thống vừa bị khởi động lại (restart) do tài nguyên máy yếu khi chạy toàn bộ dự án Bevy/Tauri. Dưới đây là tình trạng hiện tại và các công việc cần làm tiếp theo để tối ưu hóa và hoàn thiện mô hình Thỏ 3D theo phong cách Papercraft (gấp giấy có góc cạnh và viền đen).

---

## 1. Các công việc đã hoàn thành
- **Tạo Sandbox Độc lập (rabbit-standalone)**: Giúp chạy thử mô hình thỏ trực tiếp bằng HTML/Three.js mà không cần chạy backend Tauri/Rust nặng nề.
- **Sửa lỗi chân thụt thò (Leg Clipping)**: Đã áp dụng công thức co giãn chân động (Dynamic Leg Extension) theo nhịp nhảy `hopY` trên cả bản standalone và component React. Chân không còn bị tách rời hay lún sâu vào thân.
- **Nâng cấp thẩm mỹ Cute**: Đã thêm má hồng, mũi hồng, miệng và mắt bóng có tia sáng phản chiếu (eye glints).
- **Vượt qua kiểm thử**: Toàn bộ 56 frontend tests và quy trình build đều thành công (`npm run test:frontend`, `npm run build`).

---

## 2. Các phần CHƯA HOÀN THIỆN (Cần làm tiếp theo)
Theo yêu cầu tham khảo mô hình 3D mới (phong cách Papercraft/Low-poly gấp giấy):

### Tác vụ 1: Chuyển đổi mô hình sang phong cách Papercraft (Faceted Low-Poly)
- [ ] **Giảm phân đoạn hình học (Low Segment Counts)**: 
  - Chuyển các khối cầu (Sphere), khối con nhộng (Capsule) của thân, đầu, tai, chân thành các khối có số phân đoạn cực thấp (ví dụ: 4 đến 8 phân đoạn) để tạo góc cạnh sắc nét, thô ráp.
- [ ] **Áp dụng Flat Shading**: 
  - Bật thuộc tính `flatShading: true` trên tất cả các chất liệu (`MeshStandardMaterial`) để ánh sáng phản chiếu phẳng theo từng mặt đa giác, không làm mịn bề mặt.
- [ ] **Thêm viền đen sắc nét (Black Outlines)**:
  - Sử dụng `THREE.EdgesGeometry` kết hợp với `THREE.LineSegments` vẽ đè lên mỗi bộ phận để tạo viền đen chạy dọc theo các cạnh sắc của khối đa giác, mô phỏng đúng phong cách mô hình giấy gấp.

### Tác vụ 2: Điều chỉnh dáng đứng (Sitting Posture) & Tỷ lệ
- [ ] **Dáng đứng co chân**: Điều chỉnh tư thế mặc định của thỏ giống dáng ngồi khép chân trong ảnh tham khảo.
- [ ] **Tai nhọn góc cạnh**: Thiết kế lại tai thỏ sử dụng hình nón hoặc hình trụ 4 cạnh vuốt nhọn ở đỉnh.

### Tác vụ 3: Đồng bộ động học hoạt ảnh
- [ ] Cập nhật hoạt ảnh nhảy (hopping) và nhai (chewing) để hoạt động mượt mà với cấu trúc hình học Papercraft góc cạnh mới mà không làm mất đi tính tương tác của các thanh trượt điều khiển.

---

## 3. Hướng dẫn chạy nhẹ nhàng cho máy yếu (Tránh treo máy)
Để không làm quá tải CPU/RAM dẫn đến việc máy phải tự khởi động lại:
1. **KHÔNG chạy Tauri backend** (`npm run tauri dev` hoặc các lệnh Cargo).
2. **Chỉ chạy server tĩnh siêu nhẹ** phục vụ thư mục độc lập:
   ```bash
   # Đã được khởi chạy ở cổng 8000
   py -m http.server 8000
   ```
3. Mở trình duyệt truy cập: `http://localhost:8000/` để xem và tương tác trực tiếp với mô hình thỏ mà không tốn tài nguyên hệ thống.
