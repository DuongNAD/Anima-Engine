# WORLD_DESIGN.md — Thiết kế Thế giới / Map cho Anima-Engine

> Mục tiêu người dùng: *một thế giới tự nhiên, chân thực, ĐÚNG QUY LUẬT VẬT LÝ, đủ loại môi trường
> (suối, sông, biển, bờ biển, cát, hang động…). Map sinh **một lần** rồi **cache** để load lại.
> Tối ưu để chừa tài nguyên cho **hàng triệu mô hình ML** động–thực vật, chạy được trên
> Dell Vostro 3530 (i7-1355U, iGPU Iris Xe + dGPU rời) mà không full-load máy.*
>
> Tài liệu này là bản khảo sát hiện trạng + tham chiếu kỹ thuật + kiến trúc mục tiêu + roadmap.
> Ngày lập: 2026-07-24.

---

## 0. TL;DR — Kết luận cốt lõi

1. **Bạn KHÔNG cần xây map từ đầu.** Đã có một bộ sinh world rất chín (`worldGen.ts`, 2048², 22 biome,
   khí hậu rain-shadow, erosion, hồ priority-flood, sông D8, cache nhị phân IndexedDB, `WORLD_GEN_VERSION 18`).
   Về mặt **render + độ giàu môi trường**, nó đã ở đẳng cấp cao.

2. **Vấn đề #1 (kiến trúc):** map bị **TÁCH LÀM ĐÔI**. Thế giới 3D đẹp chỉ là *trang trí* trên
   `landscape.html`. Các **agent thật (ML brains) sống trong một world backend khác** — `terrain.rs`,
   chỉ **128×128** phủ **200×200 unit**, **11 biome**, và **KHÔNG được cache** (sinh lại mỗi lần chạy
   trong `init_world`, chỉ giống nhau nhờ seed hard-code `1337`). Hai bộ sinh **không chia sẻ code/dữ liệu nào**.

3. **Hệ quả:** "môi trường cho sinh vật" hiện tại là cái world 128² stub, **không phải** cái world 2048² đẹp.
   Muốn "môi trường có trước sinh vật" cho đúng, phải **hợp nhất về MỘT world quyền lực (authoritative),
   sinh 1 lần, cache ra đĩa, dùng chung cho cả render lẫn sim**.

4. **Về "hàng triệu agent":** trung thực mà nói, *hàng triệu neural brain inference/tick @60FPS* là **không khả thi**
   trên Vostro 3530. Con đường thực tế là **Simulation LOD** (chỉ agent gần/đang hoạt động chạy brain đầy đủ;
   agent xa chạy cập nhật thống kê rẻ) + **spatial hashing** + có thể **GPU-batch inference**. Nhiệm vụ của MAP
   là **rẻ và có trần tài nguyên** (chunk + LOD + SoA) để **chừa headroom** cho tầng sinh vật.

5. **Ưu tiên (theo thứ tự) & tiến độ:**
   - **M0** — ✅ **ĐÃ CHỐT hướng A→B** (Mục 4.2): trước mắt frontend-authoritative để hợp nhất nhanh + verify được; port sang Rust sau mà không đổi định dạng đọc.
   - **M1** — ✅ **Vá vật lý thủy văn XONG**: sông thoát hồ (v19) + delta cửa sông + hồ nội lưu endorheic (v20) — Mục 5.
   - **M2** — **World Artifact dùng chung**: ✅ backend sinh-1-lần + cache + reload; ✅ định dạng trung tính + codec 2 phía + chứng minh liên-ngôn-ngữ; ✅ **luồng runtime** (frontend `save_world_artifact` → backend ghi → `init_world` đọc → agent sống trên world frontend; proof end-to-end bằng world THẬT 128²); ⬜ chỉ còn **xác nhận trực quan runtime** + render 3D từ world chung (CẦN chạy app).
   - **M3** — ✅ Terrain **chunk + LOD** (frontend, opt-in, chứng minh cull 81–88% tam giác); ⬜ camera-LOD động + Simulation-LOD backend.
   - **M4** — ✅ Hang động THẬT: hốc 3D (`caveGeometry.ts`) thay decal; ⬜ hang xuyên-núi đi-vào-được cần voxel terrain.

---

## 1. Hiện trạng (đo đạc thật từ mã nguồn)

### 1a. Frontend — `src/components/Landscape/utils/worldGen.ts` (world "đẹp", nơi KHÔNG có agent)

- **Pipeline sinh** (deterministic theo `seed,size,shape`): domain-warp 8-octave **fBm** + **ridged multifractal**
  (arête) + radial falloff → **droplet hydraulic erosion** (Sebastian Lague, ≤120k hạt) → sea-level = phân vị
  histogram (đất ~38% mọi seed) → **D8 flow accumulation** (counting-sort O(n)) → **ribbon sông** thon–nở +
  meander + khoét lòng → **temperature** (vĩ độ + lapse 1.35) → **moisture** (orographic sweep 2 chiều gió +
  đai Hadley/ITCZ + rain-shadow) → **Whittaker 22 biome** → majority filter 3×3 → **hồ Priority-Flood**
  (Barnes et al., min-heap) → shoreline → **thác** (drop > ngưỡng, cap 450) → **cửa hang** (decal, cap 80) →
  **flora** (cap 90–130k) → **thủy sinh** (coral/kelp/seagrass, cap 22k).
- **Dữ liệu SoA** (`n = size²`): 7× `Float32Array` (elevation, moisture, temperature, flow, slope, water, shore)
  + 2× `Uint8Array` (riverAmt, biome) = **30 byte/cell**. → **1024² ≈ 31.5 MB**, **2048² ≈ 126 MB**
  (ghi chú "~17MB"/"~90MB" trong code/TODO đã **cũ**, cần sửa).
- **Render**: mesh terrain **384²** sample bilinear từ data 2048² (data-res tách mesh-res); màu bake vào
  **DataTexture 2048² (16MB)**, normal/roughness map res 1024 (4MB mỗi cái); tổng **~48–53MB GPU texture**.
  Toàn thế giới sống động ≈ **~30 draw call** (tất cả instanced). Có nút **Đẹp/Nhẹ** (DPR, shadow, giảm ½ instance).
- **Cache "sinh 1 lần"**: `worldCache.ts` — **IndexedDB** (`anima-world`/`worlds`), lưu **binary structured-clone**
  (không JSON), key = `world:v${WORLD_GEN_VERSION}:${seed}:${size}:${shape}`. Lần 2 đọc thẳng buffer, **bỏ qua
  thuật toán sinh hoàn toàn**. Sinh chạy trong **Web Worker** (transfer ArrayBuffer zero-copy). ✅ Yêu cầu
  "load lần đầu, sau tải lại" ĐÃ ĐẠT — nhưng chỉ cho world trang trí.
- **Không có LOD/chunk/stream**: cả 2048² luôn nằm trong RAM, 1 mesh terrain duy nhất.

### 1b. Backend — `src-tauri/src/core/terrain.rs` (world "thật", nơi agent sống)

- **Struct** `TerrainMap` (Bevy `Resource`, SoA `Vec`): `elevations, moistures, temperatures, biomes(u8), flows, pois`.
- **Sinh riêng bằng Rust**: domain-warp fBm + ridged + island falloff → **droplet erosion zero-alloc** (≤100k it)
  → temperature (vĩ độ+lapse) → moisture (fBm + flow + rain-shadow) → **Whittaker 11 biome**.
- **Độ phân giải thực dùng: 128×128, seed 1337**, phủ **200×200 unit** (`MapBounds` −100..100) ≈ 1.56 unit/cell.
- **KHÔNG cache, KHÔNG persist**: `init_world` gọi `TerrainMap::generate(&MapSettings::default())` **mỗi lần**;
  `SavedSimulationState` **không chứa** terrain/biome/seed → load save sẽ **regenerate** (chỉ trùng nhờ seed cố định).
- **Agent chạm terrain ra sao**: **gián tiếp**, qua `ResourceField` (biome → sức chứa thức ăn/cell, logistic
  regrowth). Prey gặm field tại vị trí; fruiting scale theo NPP biome. API lấy elevation/biome trực tiếp
  (`get_elevation_at_pos`, `get_map_indices`) **có mà KHÔNG ai gọi** (dead code); neural input hiện là
  raycast + pheromone + homeostasis, **không** đọc biome/elevation.
- **Quy mô hiện tại**: spawn **10 agent** lúc khởi động (7 prey/3 predator). **Không có** hằng `MAX_AGENTS`/
  target "triệu". Food cap 50. → engine hiện là **prototype vài chục agent**, "triệu agent" là *mục tiêu*, chưa là năng lực.
- **GPU**: `burn-wgpu` (fallback `ndarray`) chỉ cho **neural inference**, không cho sinh terrain.

### 1c. Sơ đồ tách rời

```
landscape.html ──> WorldShowcase.tsx ──> worldGen.ts (2048², 22 biome) ──> IndexedDB cache
                        (KHÔNG invoke backend — thuần trang trí, wildlife là prop)

index.html ─────> PixiViewport.tsx ──(get_terrain_map)──> TerrainMap (128², 11 biome)  ──> agent ML sống ở đây
                        └── minimap 2D dùng terrain backend            └── ResourceField (thức ăn theo biome)
```

Có **`terrainGenerator.ts`/`terrainCache.ts` + `LandscapeShowcase.tsx`** là **stack legacy** (160², AoS, DB
`anima-landscape` v2) đã bị `worldGen.ts` thay thế — nên **gỡ bỏ** để tránh nhầm lẫn/bảo trì kép.

---

## 2. Khoảng trống về "đúng vật lý" & "đủ môi trường"

| Vấn đề | Chi tiết | Ảnh hưởng |
|---|---|---|
| **Hang động chỉ là DECAL** | `WorldCaves.tsx`: ellipse phẳng, material đen unlit áp lên vách. Không nội thất, không thể vào, không collision cavity. Cap 80, cấm vùng tuyết. | "Hang động" trong yêu cầu **chưa có thật**. |
| **Sông chỉ là dải texture trên mặt** | `riverAmt` bake vào màu + shimmer shader; khoét lòng 0.0035 (rất nông). Không tiết diện 3D, không discharge thật. | Nhìn đẹp nhưng không có chiều sâu vật lý. |
| **D8 single-flow mong manh** | Sinh rãnh song song theo chéo lưới → phải "vá" bằng ngưỡng/gate nhiều lần (lịch sử v13/v16/v17). | Mạng thoát nước dễ vỡ, không robust. |
| **Hồ KHÔNG thoát nước** | `computeLakes` (priority-flood) biết cao độ sill nhưng **không route sông tràn** từ mỗi bồn. (Bản legacy từng làm — bản SoA bỏ mất.) Không có **delta** nơi sông ra biển. | **Vi phạm bảo toàn nước** — điểm "sai vật lý" rõ nhất. |
| **Khí hậu 2 cực đối xứng** | `lat = 1-\|ny\|` → xích đạo ở giữa map, cực ở **cả hai** biên; không có Bắc–Nam thật, không mùa, không dòng hải lưu. | Đủ đẹp, chưa "địa cầu". |
| **Gió chỉ theo trục Đông–Tây** | Orographic sweep chỉ chạy dọc hàng X → rain-shadow chỉ tạo qua dãy Đông–Tây. | Bóng mưa thiếu chiều Bắc–Nam. |
| **Erosion 1 lượt, chỉ fluvial** | Không thermal/talus, không routing trầm tích thành fan/delta, không kiến tạo/isostasy. | Địa mạo tĩnh. |
| **Không LOD/stream** | 2048² (~126MB) + ~50MB texture luôn resident; 1 mesh 384² → silhouette chân trời thô, khó scale >2048². | Chặn đường mở rộng + nhường tài nguyên cho agent. |
| **Trần cứng nhiều thứ** | thác 450, hồ 520, hang 80, flora 90–130k, thủy sinh 22k. | World dày sẽ bị cắt cụt. |
| **Đáy biển sâu trống** | Không gì dưới `depth<0.085`. | Thiếu môi trường biển sâu. |
| **Trùng lặp 2 stack sinh** | `worldGen`↔`terrain.rs` + legacy `terrainGenerator`. | Nguy cơ bảo trì/nhất quán. |

---

## 3. Tham chiếu kỹ thuật (các dự án & nghiên cứu để "làm map tốt nhất")

Đúng như yêu cầu, đây là các nguồn thực tế để nâng từng mảng:

- **Thủy văn hạt (particle-based) — Nick McDonald, "Procedural Hydrology"** (nickmcd.me, 2020).
  Mô phỏng giọt nước xói mòn + **flood-fill tạo hồ** + **cascade** (bồn cao tràn xuống bồn thấp) trên chung 3 map
  (height/stream/pool). Điểm mạnh: **sông và hồ sinh ra từ CÙNG một quá trình vật lý** → tự nhất quán (sông đổ vào hồ,
  hồ tràn thành sông, thác ở điểm tràn). Tham số: `evapRate`, `depositionRate`, `volumeFactor≈100`, `lrate≈0.01`.
  → *Đây là hướng vá gốc rễ cho Mục 2 (hồ/sông/thác), thay bộ ba D8+threshold+drop rời rạc hiện tại.*
- **Terrain sinh TỪ mạng thủy văn — Génévaux et al., ACM ToG 2013**, "Terrain Generation Using Procedural Models
  based on Hydrology". Dựng **drainage network trước** (graph phân cấp) → phân loại watercourse → dựng địa hình quanh
  sông bằng blend/carve. **Bảo đảm nhất quán thủy văn theo thiết kế** (không cần "vá" như D8). *Tham chiếu cho bản
  đại tu hydrology dài hạn.*
- **Chunked LOD + Simulation LOD** (nhiều nguồn game/engine): quadtree heightmap, tessellation tối ưu; **giảm tần suất
  cập nhật agent xa/không liên quan**; batch-loading giới hạn thời gian/frame. *Nền cho M3 — chừa tài nguyên cho triệu agent.*
- **Hang động THẬT**: **Cellular automata trên voxel grid → scalar field → Marching Cubes** (overhang/hang/tunnel).
  Heightmap không thể có hang/overhang; cần **hybrid**: giữ heightmap cho bề mặt, chỉ **voxel cục bộ + marching cubes**
  ở nơi có hang (thưa) rồi **cache mesh**. *Tham chiếu cho M4.* (Bộ ref: AK-Saigyouji Procedural-Cave-Generator; PLUME 2025.)
- **Dự án ALife tham chiếu — The Bibites, Framsticks**: thế giới dùng **grid resource field** + **spatial partitioning**
  (uniform grid/loose octree) — chính là mô hình `ResourceField` + `SpatialHashGrid` bạn đã có. Đúng hướng; việc còn lại
  là **scale + LOD**.

*(Danh sách URL đầy đủ nằm trong phần trả lời chat kèm commit này.)*

---

## 4. Kiến trúc mục tiêu: MỘT "World Artifact", sinh 1 lần, cache, dùng chung

### 4.1. Định dạng "World Artifact" (nguồn sự thật duy nhất)

Một khối SoA nhị phân, versioned, **generate-once → serialize → cả render lẫn sim đọc chung**:

```
WorldArtifact {
  version, seed, size, worldScale, seaLevel,
  elevation:  f32[size²]   // đã erosion, đơn vị chuẩn hoá
  biome:      u8 [size²]   // MỘT taxonomy chung (đề xuất 22)
  moisture,temperature: f32[size²]
  water:      f32[size²]   // mực nước hồ/biển (0 = khô)
  flow/riverAmt: u8/f32[size²]
  resourceCapacity: f32[size²]  // R_max (NPP) — sim đọc trực tiếp, khỏi suy lại từ biome
  // sparse: lakeBasins[], waterfalls[], caves[], (delta[]) …
}
```

Lưu **ra đĩa** (app data dir), key = `v{version}:{seed}:{size}` → lần đầu sinh, lần sau `mmap`/đọc thẳng.
Cả frontend (render) và backend (sim) cùng key → **đảm bảo agent sống đúng trên world đang được vẽ**.

### 4.2. Hai phương án hợp nhất (M0 — cần bạn chốt)

**Phương án A — Frontend `worldGen.ts` là authoritative.**
`worldGen.ts` sinh + export WorldArtifact ra file; backend đọc file lúc `init_world` (thay `TerrainMap::generate`).
- ➕ Tận dụng ngay bộ sinh giàu nhất (22 biome, rain-shadow, hồ, sông) — ít phí công.
- ➕ Verify được trên máy yếu (frontend build + smoke + screenshot headless).
- ➖ Sim (Rust headless) **phụ thuộc artifact do JS sinh** — chạy sim buộc phải sinh world qua frontend/worker trước.

**Phương án B — Backend Rust là authoritative.**
Port thuật toán giàu của `worldGen.ts` sang Rust (hoặc nâng `terrain.rs` lên ngang), sinh ở res cao, cache ra đĩa;
frontend đọc artifact đó để render (bỏ `worldGen.ts` khỏi đường sinh).
- ➕ Sim tự chủ, đúng "engine headless"; hợp cho hiệu năng triệu-agent (Rust, rayon, zero-alloc).
- ➕ Một nguồn sự thật nằm ở nơi hiệu năng quan trọng nhất.
- ➖ Tốn công port TS→Rust; khó verify hình ảnh trên máy yếu.

> **Khuyến nghị:** **Phương án B là đích dài hạn đúng đắn** cho một ALife engine (sim phải tự chủ, hiệu năng là ở Rust).
> Nhưng **thực dụng trước mắt: đi A→B**. Giai đoạn 1 dùng A (export artifact từ `worldGen.ts`, backend đọc) để **hợp nhất
> nhanh + verify được ngay**; song song **chuẩn hoá taxonomy biome + định dạng artifact**. Khi ổn định, **port bộ sinh
> sang Rust (B)** dùng chính artifact-format đó — frontend không phải đổi vì chỉ đọc artifact. Đổi "ai sinh" mà **không đổi
> "định dạng đọc"**.

### 4.3. Ngân sách tài nguyên (Vostro 3530) & đường tới nhiều agent

- **RAM map**: WorldArtifact 2048² ≈ **~130MB** (chấp nhận — "map có thể ăn nhiều RAM"). Nếu cần lớn hơn → **chunk/stream**.
- **GPU map**: giữ **trần ~50–60MB texture** + ~30 draw call (đã đạt). Nút Nhẹ cho iGPU.
- **Chừa cho agent**: map **KHÔNG được chạy mỗi tick** (chỉ sinh 1 lần + regrowth ResourceField rẻ). CPU/GPU 60FPS
  phải dành cho brain inference.
- **Nhiều agent = Simulation LOD**, không phải brute force: agent trong "active radius" chạy brain đầy đủ; ngoài đó chạy
  cập nhật quần thể thống kê (như cách ecology E1–E11 đã mô hình hoá dòng năng lượng). Đây là cách duy nhất tiệm cận
  "triệu" trên phần cứng này. Map cần **spatial index theo chunk** để bật/tắt LOD theo vùng.

---

## 5. Roadmap tuần tự (mỗi bước verify được, giữ luật zero-alloc backend & cache frontend)

- **M0 — Chốt hướng authoritative.** ✅ **ĐÃ CHỐT: A→B** (frontend giàu làm nguồn trước, port sang Rust sau, giữ nguyên định dạng đọc). Còn: chuẩn hoá taxonomy biome chung (11↔22) khi làm M2 phần chia sẻ.
- **M1 — Vá vật lý thủy văn (bộ sinh giàu nhất).**
  - ✅ **[XONG v19]** Route **sông tràn từ mỗi hồ** (Priority-Flood → BFS plateau tìm pour-point → steepest-descent
    trên `filled,elev` → stamp `riverAmt`+`Biome.River`). Hồ→sông→biển/cascade nhất quán, bảo toàn nước.
  - ✅ **[XONG v20] Delta cửa sông** (Pass 4d): sông gặp biển bồi cát theo quạt flow-scaled vào vùng nông → cồn cát Beach nổi.
  - ✅ **[XONG v20] Bồn nội lưu (endorheic)** (Pass 4b-2 per-basin): bồn khô (moisture TB < 0.24) KHÔNG thoát nước → hồ tận
    `saline` + viền salt-flat; bồn ẩm vẫn spillway. `computeLakes` trả `outletPaths` per-basin. 4/25 hồ nội lưu @1024² (~16%).
    Verify: smoke 0 NaN/22-22/đất 38% · cargo 36/36 · vitest 21/21+237/237 · lint 0 · build ✅.
- **M2 — WorldArtifact + world dùng chung.**
  - ✅ **[XONG] Backend sinh-1-lần + cache đĩa + reload**: `TerrainMap: Serialize/Deserialize`;
    `TerrainMap::load_or_generate(settings, cache_dir)` (bincode, key = settings+config+`WORLD_CACHE_VERSION`); `init_world`
    dùng nó (override `ANIMA_CACHE_DIR`). 3 cargo-test + clippy sạch. → thoả "sinh 1 lần, lần sau tải lại" cho thế giới agent.
  - ✅ **[XONG] Định dạng "World Artifact" ngôn ngữ-trung-tính + CHỨNG MINH liên-ngôn-ngữ**: binary LE phẳng (magic `ANMW`,
    header + `elevation/moisture/temperature/flow` f32 + `biome` u8) — **KHÔNG dùng bincode Rust-only**. Cài cả hai phía:
    Rust `core/world_artifact.rs` (`WorldArtifact::from_bytes/to_bytes`, `to_terrain_map` downsample + **map biome 22→11**) và
    TS `utils/worldArtifact.ts` (`encode/decodeWorldArtifact`). **Fixture do encoder TS ghi ra** (`scripts/gen_artifact_fixture.ts`)
    được **cargo-test đọc lại và assert byte-cho-byte khớp Rust** (`decodes_frontend_generated_fixture`) → hai phía **đồng thuận
    định dạng** mà không cần chạy app. `init_world` đọc `ANIMA_WORLD_ARTIFACT` → agent sống trên **CHÍNH world frontend sinh ra**
    (fallback cache nếu vắng). Verify: cargo `world_artifact` 5/5 · src Vitest `worldArtifact` 3/3 · lib 34/34 · clippy 0 · build ✅.
  - ✅ **[XONG] Luồng runtime ghép hai world** (verify luồng DỮ LIỆU end-to-end, không cần app): frontend
    `worldCache.loadOrGenerateWorld` → `worldToArtifact(world, 256)` → `invoke('save_world_artifact', bytes)`; backend command
    `save_world_artifact` **validate + ghi** ra `default_artifact_path()` (env `ANIMA_WORLD_ARTIFACT`, mặc định temp); `init_world`
    đọc đúng path đó → `to_terrain_map` → **agent sống trên world frontend sinh ra** (fallback cache nếu chưa có). **Bằng chứng
    end-to-end:** cargo test `real_frontend_world_becomes_valid_terrain_map` — world THẬT 128² từ `worldGen.ts` (fixture do frontend
    encode) → TerrainMap backend hợp lệ **có cả biển lẫn đất**. Verify: cargo lib **36/36** · clippy 0 · src Vitest **21/21** ·
    test:frontend 237/237 · lint 0 · build ✅.
  - ⬜ Còn lại (CẦN CHẠY APP): xác nhận trực quan dòng `invoke('save_world_artifact')` chạy thật trong Tauri + sim đọc đúng;
    tuỳ chọn 3D showcase render TỪ world chung; đưa terrain/`MapSettings` vào `SavedSimulationState`; thêm `resourceCapacity`/water
    vào artifact (bump version). *Thuật toán + luồng dữ liệu đã xong; chỉ còn xác nhận trực quan runtime.*
- **M3 — Khung chunk + LOD + simulation-LOD.**
  - ✅ **[XONG] Terrain chunk + LOD ĐỘNG theo camera** (`utils/chunkLod.ts` + `WorldTerrainLod.tsx`, opt-in `TERRAIN_CHUNKED`):
    lưới C×C, frustum-cull vùng ngoài màn hình + LOD hạ chi tiết chunk xa (theo camera, cache geometry per (chunk,lod) + swap
    imperatively, throttle) + skirt che khe. Chứng minh SỐ: uniform = trùng khít single-mesh (294912 tam giác, seam 0.0);
    walk/fly vẽ **12–19%** tam giác; LOD động tái tâm chi tiết khi camera đổi góc (harness + vitest). (Live LOD chờ 1 lần nhìn máy thật.)
  - ✅ **[XONG] Streaming** (`updateActiveChunks`): chỉ giữ chunk gần camera resident, thả + dispose chunk xa (hysteresis
    chống thrash) → **trần bộ nhớ cố định** = đường mở world lớn hơn 1 mesh. Harness: quét world 256-chunk → tối đa 22 resident (9%), 0 NaN.
  - ⬜ Còn lại: **Simulation-LOD backend** (agent trong active-radius chạy brain đầy đủ, ngoài đó cập nhật thống kê) —
    ở backend, nên phối hợp với phiên đang sửa backend.
- **M4 — Hang động THẬT.**
  - ✅ **[XONG]** Hốc hang 3D thật (`caveGeometry.ts` — funnel đá noise-displaced, vertex-AO tối dần vào trong, 1 mesh gộp)
    thay decal phẳng trong `WorldCaves.tsx`. Verify: builder standalone PASS (0 NaN/0 index lỗi, ngồi đúng vách), tests/build ✅.
  - ⬜ Còn lại (nặng, tuỳ chọn): hang **xuyên núi / overhang đi-vào-được** — cần **voxel/SDF terrain cục bộ + marching cubes**
    tại cửa hang + đục lỗ mesh terrain + collision cavity cho explore-mode. Heightmap liên tục hiện tại không đục lỗ được ở res 384.
- **M5 (tuỳ chọn) — Đại tu hydrology theo Génévaux** (drainage-network-first) nếu muốn "đúng vật lý" ở mức nghiên cứu.

---

## 6. Việc dọn dẹp kèm theo (nợ kỹ thuật đã phát hiện)

- ⚠️ **ĐÍNH CHÍNH (2026-07-24):** stack `LandscapeShowcase.tsx` + `terrainGenerator.ts` + `terrainCache.ts` + `Terrain/Water/
  Vegetation/Sky/Weather/Minimap` **KHÔNG chết** — `LandscapeShowcase` được **`src/App.tsx` import** (viewport landscape trong app
  chính, khác `landscape.html`) và là nền của ~một nửa bộ 237 test (`landscape_showcase*`, `terrainGenerator`, `terrainWater`,
  `vegetation`, `skyWeather`). `worldGen.ts` cũng **vẫn dùng `ImprovedNoise2D` từ `terrainGenerator`**. → **KHÔNG gỡ** (sẽ vỡ app +
  test). Nếu muốn thống nhất một stack landscape thì phải chuyển App.tsx sang WorldShowcase + viết lại test — một việc lớn, cần
  nhìn tận mắt, không phải "dọn dẹp".
- Sửa số liệu cũ: `worldCache.ts:9` ("~17MB" → ~31MB @1024²); `TODO.md:381` ("~90MB" → ~126MB @2048²).
- Xoá/đánh dấu dead code backend: `get_elevation_at_pos`, `get_map_indices` (0 caller) — sẽ dùng khi agent đọc biome trực tiếp.
- Đồng bộ `PROJECT.md` Interface Contracts (thiếu `get_terrain_map`, `get_environmental_elements`).

---

## 7. Simulation-LOD (backend) — ĐÃ THỰC THI ĐỦ HAI TẦNG

> Mảnh ghép thật sự cho "hàng triệu agent" trên máy yếu. Ở **backend Rust**.
> Thiết kế dưới đây là bản gốc; phần **đã thực thi** và phần **còn nợ** ghi ở mục "Trạng thái" cuối mục.

**Nguyên lý:** brain inference (Burn) là chi phí trội; không thể chạy đủ cho triệu agent @60FPS trên Vostro 3530.
Giải pháp là **phân tầng cập nhật theo khoảng cách tới tiêu điểm (camera/observer) hoặc theo mức "đáng quan tâm"**:

- **Tầng HOT (active-radius):** agent gần tiêu điểm → chạy **brain đầy đủ mỗi tick** (như hiện tại).
- **Tầng WARM:** agent xa hơn → **brain thưa** (mỗi N tick / hạ tần số) + vật lý đơn giản hoá.
- **Tầng COLD:** rất xa/không quan sát → **KHÔNG chạy brain**; cập nhật bằng **mô hình quần thể thống kê** trên chunk
  (dùng chính khung năng lượng khép kín E1–E11 đã có: sinh–tử–ăn theo Lotka-Volterra/logistic ở mức tổng hợp mỗi ô).
  Khi observer lại gần → **"nở" (re-hydrate)** vài cá thể đại diện từ thống kê ô đó.

**Điểm tựa sẵn có (khi ổn định sẽ kiểm lại):** `SpatialHashGrid` (đã có) để phân vùng theo chunk; `ResourceField` +
`EcosystemBiomass` (đã có, bảo toàn năng lượng) làm sổ cái tầng COLD; chunk grid của M3 (`chunkLod.ts`) làm lưới LOD chung
render↔sim. Mỗi tick: chỉ lặp brain cho agent tầng HOT; tầng WARM theo bộ đếm; tầng COLD chạy 1 bước logistic/ô.

**Ràng buộc bắt buộc:** giữ **zero-alloc hot path** (buffer tiền cấp phát cho phân tầng, không cấp phát trong tick).
**Verify (không cần chạy app):** `cargo test` — (1) bảo toàn năng lượng khi COLD↔HOT chuyển tầng (tổng năng lượng bất biến),
(2) agent HOT giữ hành vi cũ, (3) re-hydrate không tạo/mất năng lượng, (4) `allocs == 0` trong tick. `cargo clippy` sạch.

### Trạng thái (2026-07-25)

**✅ Tầng 1 — phân tầng nhịp suy nghĩ. Đã thực thi:** `src-tauri/src/core/simulation_lod.rs`, hook trong
`sensory_system` ngay sau chốt `CognitiveState::Ready`.

- HOT/WARM/COLD chia theo khoảng cách tới `LodFocus`; mặc định `hot_radius=50`, `warm_radius=100`, `warm_interval=8`.
- WARM **so le theo entity index** (`(tick + index) % interval`), không dồn cả dải vào một tick — cùng tổng công việc
  nhưng đến thành gai mà khung hình chịu còn số trung bình giấu đi. Cùng thủ thuật với so-le trường tài nguyên.
- **Mặc định tắt.** `LodFocus::enabled = false` ⇒ mọi agent HOT ⇒ không phân biệt được với bản không có module.
  Mọi run headless hôm nay rơi vào nhánh này.
- Trả nốt một lời hứa của ADR-0003: `LifetimeLearning.active_radius` nay đo từ tâm LOD thay vì gốc toạ độ
  (gốc toạ độ là chỗ đứng tạm để ràng buộc kiểm được khi chưa có gì để lấy làm tâm).
- Không cấp phát trong tick: `LodGate` là `SystemParam` trên stack, `LodSnapshot` là `Copy`.
- Gate: 9 unit + 9 integration (`tests/simulation_lod_tests.rs`), clippy sạch.

**✅ Tầng 2 — quần thể thống kê + re-hydrate. Đã thực thi:** `src-tauri/src/core/aggregate_population.rs`.

Agent ngủ bị **huỷ hẳn** (entity, component, não). Còn lại là mấy con số trong một chunk (lưới 32×32).

- **Bảo toàn năng lượng — một dòng, một chỗ.** Con vật ngủ **vẫn là con vật**: năng lượng của nó chưa bị ăn, chưa
  hô hấp, chưa về mùn — chỉ thôi được phân giải theo từng cá thể. Nên **không có compartment thứ tư** và **không có
  giao dịch ledger** nào: `ecosystem_census_system` cộng thêm `DormantCohorts::total_energy()` vào `pool.animals`,
  hết. Phương án thay thế (dồn dự trữ vào mùn) cũng bảo toàn EU nhưng **sai sinh thái** — đi khỏi một bầy thú là
  bón phân cho cả vùng.
- **Nhả năng lượng và huỷ body dùng chung một sync point.** `DehydrateAgentCommand` là `Command` đúng vì lý do
  `ReclaimAndDespawnAgentCommand` đã ghi: nếu ghi sổ trong system rồi despawn qua `Commands`, còn một khoảng trong
  đó body vẫn sống mang dự trữ đã bị đếm ở chỗ khác → mỗi lần ngủ là thế giới đẻ ra một suất EU.
- **Trễ (hysteresis) 120 tick.** Không có nó, agent chạm mép vùng warm là mất não ngay tick đầu, và một observer
  lia camera qua lại sẽ **điều khiển tiến hoá**.
- **Đa dạng di truyền: có giới hạn, và được đếm.** Mỗi chunk giữ tối đa 8 genome, lấy **mẫu ngẫu nhiên đều**
  (reservoir, thuật toán R) chứ không phải "8 đứa đến sau cùng" — mẫu theo thứ tự đến là chọn lọc theo thời điểm
  đến, vô nghĩa về sinh học. Hệ quả nêu thẳng: **cohort ngủ có kích thước quần thể hiệu dụng = 8**, trôi dạt di
  truyền ở đó nhanh hơn quần thể sống. `genomes_dropped()` đếm đúng số genome đã mất.
- **Lossless dưới cap.** Cohort chưa quá 8 cá thể thì trả lại **đúng** những genome đã nuốt. Trên cap là mất mát
  có chủ đích. Ranh giới đó là một test, không phải một ghi chú.
- Gate: 22 unit + 21 integration (`tests/aggregate_population_tests.rs`). Gate năng lượng G1.1 và gate determinism
  vẫn xanh sau khi sửa census.

**Tiết kiệm bộ nhớ là CÓ ĐIỀU KIỆN — nói rõ vì dễ tưởng nhầm:**

| Cohort | Archive giữ gì | Tiết kiệm |
|---|---|---|
| ≤ 8 cá thể | **đủ mọi genome** | chỉ body ECS + mạng `learned` — cỡ hệ số 2, không hơn |
| > 8 cá thể | 8 genome | không chặn trên: bộ nhớ ngủ thành O(số chunk), không còn O(số agent) |

Nghĩa là vài agent đi khuất tầm mắt gần như **không** tiết kiệm gì. Trần bộ nhớ chỉ nhúc nhích ở quy mô mà một chunk
vượt cap — đúng cái quy mô mà mục tiêu "hàng triệu agent" sống, và cũng đúng cái quy mô mà ngủ trở nên mất mát.
Hai điều đó là **cùng một điều**.

**✅ Sinh thái tổng hợp cho cohort ngủ — đã thực thi:** `dormant_cohort_ecology_system`.
Thời gian **có** trôi ở nơi không ai nhìn: cohort hô hấp, và phần ăn cỏ trong đó gặm `ResourceField` dưới chunk của
mình. Cả 4 điều kiện verify liệt kê ở trên đều xanh.

Ràng buộc chi phối không phải "mô hình sinh thái cho hay", mà là **đừng trở thành một sinh thái thứ hai, khác đi** —
mọi chênh lệch giữa hai mô hình chính là sự chú ý của observer rò vào sinh học. Hai hệ quả gánh chịu điều đó:

- **Trao đổi chất là ĐO, không phải mô hình hoá.** Cohort đốt đúng tốc độ mà thành viên của nó *đã được đo* khi còn
  là body sống (`FeatureTracker`, trung bình mỗi tick của epoch hiện tại). Viết một công thức maintenance-only là
  cách tự nhiên nhất và sẽ làm **ngủ rẻ hơn bị nhìn**, khiến vùng không ai quan sát âm thầm nuôi được quần thể lớn hơn.
- **Không có chết đói.** Agent sống ở 0 năng lượng không bị despawn (`update_agent_evaluation_system` chỉ ngừng đếm),
  nên cohort đói chạm đáy 0 và **giữ nguyên số thành viên**. Thêm chết theo mật độ trông như sinh thái phong phú hơn,
  thực chất là observer chọn ai chết.

**Cái bẫy mà phép gộp giăng ra — ghi lại vì nó chạy trơn tru và sai âm thầm.** Cách hiển nhiên là đưa cho
`herbivore_intake` **tổng** tài nguyên của cả chunk. Nó biên dịch được, bảo toàn năng lượng chính xác, mọi test bảo
toàn đều qua. Nhưng Holling Type II bão hoà theo **mật độ**: tổng của ~64 ô nằm sâu trong vùng bão hoà hơn bất kỳ ô
đơn lẻ nào mà agent sống đứng lên, nên **đàn ngủ ăn giỏi hơn đàn được nhìn**. Đáp ứng chức năng là *trên đầu cá thể*
nên phải áp *trên đầu cá thể*: mỗi cá thể ngủ gặm như đang đứng trên một ô **trung bình** của chunk, rồi mới nhân với
số con ăn cỏ. Test `sleeping_is_not_cheaper_than_being_watched` là thứ đã bắt được nó, và là thứ giữ nó.

**⚠️ Điều kiện bắt buộc — CHƯA persist.** `SavedSimulationState` rất kỹ về năng lượng khép kín (mang theo
detritus/plants/animals, từng ô `resource_field_r`, cả **vị trí rút** của RNG) đúng để ranh giới save/load không
sinh hay huỷ EU. Nhưng nó **không** mang `DormantCohorts`. Vậy save một run đang có cá thể ngủ sẽ **âm thầm xoá**
quần thể đó cùng năng lượng của nó: chúng không nằm trong `agents`, EU của chúng không nằm trong scalar nào, và
thế giới nạp lại đơn giản là ít đi. Không gì phát hiện được, vì baseline mới sẽ khoá ở lần census đầu sau khi nạp.

→ **Đừng bật dormancy trên run có save, cho tới khi cohort vào được snapshot envelope.** Hôm nay an toàn chỉ vì
không chỗ nào chèn resource đó — tầng này tắt trong mọi đường đã ship, và tiêu điểm LOD từ UI (thứ sẽ bật nó lên)
chưa tồn tại. Ai nối tiêu điểm đó thì sở hữu dòng này.

**Phần thô hơn, và một bất đối xứng còn để ngỏ — nêu tên thay vì giấu:**

- Đàn ngủ không được phân giải tới vị trí trong chunk, nên nó gặm các ô theo tỉ lệ tài nguyên từng ô chứ không chọn ô.
  Hành vi phân tán theo giving-up density của thú ăn cỏ sống không có bản tương ứng ở mức tổng hợp.
- **Chưa có ăn thịt ở mức tổng hợp.** Trong thế giới sống, `combat_system` chuyển EU từ mồi sang thú săn và thải phần
  còn lại về mùn theo hiệu suất Lindeman — tức chuỗi thức ăn sống *rò* năng lượng xuống dưới. Cohort ngủ gộp dự trữ
  của cả hai lớp vào một con số, nên phép chuyển đó thành vô hiệu và phần thất thoát Lindeman không xảy ra: một chunk
  có cả hai lớp bảo toàn năng lượng **tốt hơn một chút khi ngủ so với khi thức**. Hướng lệch đã biết, độ lớn bị chặn
  bởi tốc độ ăn thịt. Để ngỏ chứ không lấp bằng một mô hình gặp gỡ tự nghĩ ra — một tốc độ gặp gỡ sai sẽ là thứ
  phụ-thuộc-observer tệ hơn cái nó vá. Cohort thuần một bậc dinh dưỡng (trường hợp phổ biến) không bị ảnh hưởng.
