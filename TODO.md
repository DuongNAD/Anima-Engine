# Danh sách công việc & Kế hoạch tiếp theo (TODO)

> ⬇️ Mục đang làm: **World Terrain Overhaul** (mới nhất: Khí hậu thật v11 ngay dưới). Phần "Mô hình Thỏ Papercraft" cũ được giữ lại ở cuối file như lưu trữ.

---

# 🦌 [MỚI NHẤT] v20 — ĐỘNG VẬT: cá hồ, vịt, diệc, bướm, hươu, dê núi (2026-07-03)

Yêu cầu: "thêm sinh vật trên cạn và dưới nước, đặc biệt ở sông suối ao hồ". **KHÔNG bump WORLD_GEN_VERSION** (render-side — cache giữ nguyên).

- **Cá nước ngọt** (`WorldFish` mở rộng): mỗi bồn hồ ≥90 cell² chưa đóng băng nhận 1 đàn (tối đa 10 đàn hồ) — bơi vòng giữa tầng nước dưới mặt hồ, màu nước ngọt (bạc/nâu vàng/xám xanh). Đã thấy đàn cá lượn giữa hồ núi lớn trong screenshot.
- **`WorldWildlife.tsx` mới** (5 instanced mesh, ~170 con): 
  - **Vịt** (≤48): 2–4 con/bồn hồ ấm, nổi tại mực tràn +0.05, trôi vòng chậm r 1.2–2.8 + bob; đầu xanh két/thân nâu/mỏ vàng (vertex paint).
  - **Diệc** (≤32): đứng bất động ở bờ hồ (`shore>0.85`) hoặc bờ sông (riverAmt láng giềng >150), đất thoải ấm; chân que + cổ nghiêng + mỏ vàng.
  - **Bướm** (≤52): rập rờn quanh bờ nước + đồng hoa ấm (lissajous + flap scale-Y 11Hz), 4 màu (trắng/vàng cam/hồng/xanh), meshBasic 2 tam giác.
  - **Hươu** (≤46): đàn 2–4 con trên Grassland/Shrubland/Savanna/Forest thoải (slope<0.25), geometry thân+4 chân+cổ+đầu, bob gặm cỏ nhẹ, castShadow.
  - **Dê núi** (≤16): cùng geometry tint trắng, trên Alpine/Rock slope 0–0.6.
- Placement deterministic (hash probe trên field world) — mỗi lần vào map thú vẫn ở đúng "lãnh thổ" cũ.
- **Verify:** build ✅ · 7/7 & 237/237 ✅ · lint 0 lỗi · screenshot: đàn cá giữa hồ ✓, không lỗi console. (Mật độ là cap — chỉnh trong WorldWildlife nếu muốn nhiều/ít hơn.)

---

# 🐠 v19 — HỆ SINH THÁI DƯỚI NƯỚC: san hô, kelp, cỏ biển, đàn cá (2026-07-03)

Yêu cầu: "làm cả hệ sinh thái dưới nước". **Bump `WORLD_GEN_VERSION` 17→18** (flora thêm loài thủy sinh).

- **3 loài thủy sinh mới** (FloraType 11–13, Pass 5b trong `worldGen`): **Coral** (thềm nhiệt đới t>0.6, độ sâu 0.018–0.055, dens 0.24 — mảng rạn patchy), **Kelp** (ôn đới t 0.28–0.6, cao 2 unit), **Seagrass** (dải nông nhất ≤0.018). Cap 22k; @2048²: Coral 10.3k · Kelp 8.3k · Seagrass 3.4k. Mọc trên đáy biển (mesh terrain dưới nước), nhìn xuyên qua nước nông trong suốt; KHÔNG castShadow.
- **Màu san hô**: geometry base nhạt trung tính + instanceColor 5 tông (hồng/cam/tím/đỏ/kem) theo hash — như cơ chế hoa dại.
- **Đàn cá** (`WorldFish.tsx` mới): 12 đàn × 20 con = 1 InstancedMesh (1 draw call); site dò deterministic trên thềm nắng (depth 0.02–0.07); bơi vòng giữa tầng nước (floor↔mặt), mỗi con lệch pha + bob dọc + vẫy đuôi (scale-x sin); mỗi đàn 1 màu (bạc/vàng/xanh/cam/ngọc/hồng); instanceColor set 1 lần (t<0.5s).
- **Đáy nông = cát sáng** (bake): Ocean cell depth<0.06 blend về màu cát (ấm hơn ở vùng nóng) → nước ven bờ PHÁT màu ngọc lam, san hô nổi bật trên nền cát.
- **Nước nông trong hơn**: alpha shallow 0.4→0.32.
- **Verify:** build ✅ · 7/7 & 237/237 ✅ · lint 0 lỗi · smoke @2048²: 4.0s, flora tổng **117.7k**, 22/22 biome, 0 NaN · screenshot reef: rạn màu qua nước ngọc lam + đàn cá silhouette — 0 lỗi console.
- **Tinh chỉnh:** dải san hô → gates depth/temp trong Pass 5b; mật độ → dens 0.24/0.11/0.16; số đàn cá → props `schools/fishPerSchool`; độ trong nước → alpha `0.32`; màu san hô → bảng tint trong WorldVegetation.

---

# 🌊 v18 — Sông chảy shimmer + sóng vỗ bờ + bóng mây trôi + núi răng cưa (2026-07-03)

**Bump `WORLD_GEN_VERSION` 16→17** (ridge weight đổi → relief mới).

- **Sông CHẢY thật** (`WorldTerrain` onBeforeCompile mở rộng): thêm `uRiverMask` (texture R8 từ `riverAmt`) + `uTime` (shaderRef cập nhật trong useFrame) — 2 lớp detail-noise trượt ngược chiều nhân vào diffuse trên mặt sông (`riv*0.3`) → nước lấp lánh trôi, kết hợp roughness glint.
- **Sóng vỗ bờ thở nhịp** (`WorldWater` fragment): độ với của dải foam dao động `1.5 + surf*0.55` với `surf` = 2 sóng sin chạy dọc bờ (pha theo x+z và x−z) → mép biển phồng-xẹp lan từng đoạn như sóng cuộn; hồ dùng chung shader nên cũng có sóng nhẹ.
- **Bóng mây trôi**: lõi mỗi cụm mây bật `castShadow` — bóng elip mềm quét qua đồng cỏ khi mây trôi (mây trong shadow camera ±0.7·worldScale). Nếu bóng quá gắt trên máy thật: tắt bằng cách bỏ `castShadow` trong WorldSky.
- **Núi răng cưa hơn**: ridge weight `0.5→0.58` — sống núi arête sắc, đỉnh serrated (kéo theo +2 hồ băng do đỉnh cao lạnh hơn — logic tự nhất quán).
- **Verify:** build ✅ · 7/7 & 237/237 ✅ · lint 0 lỗi · smoke @2048²: 3.97s, 22/22 biome, flora 95.7k, 450 thác, 50 hang, 28 hồ (5 đóng băng), 0 NaN · screenshot 0 lỗi console; snowline preview lởm chởm tự nhiên quanh khiên băng.
- **Tinh chỉnh:** cường độ shimmer → `riv*0.3` + tốc độ scroll trong patch; biên độ sóng bờ → `0.55`; độ sắc núi → ridge `0.58`.

---

# 🏔 v17 — Theo báo cáo phân tích tự nhiên: snowline mềm, rừng ven sông, sông thon-nở (2026-07-03)

User gửi báo cáo phân tích 4 nhóm lỗi so với tự nhiên (thủy văn/bờ nước/biome/địa mạo). **Bump `WORLD_GEN_VERSION` 16** (code: 15→16). Đã xử lý:

- **Đường song song trên núi tuyết** (thủy văn #1): thủ phạm là ribbon vẫn **khoét lòng dưới nắp tuyết** → rãnh song song lộ qua normal map/AO. Fix: gate `tApprox < 0.2` (ước lượng nhiệt lapse ngay tại Pass 2b) — vùng đóng băng không có sông lỏng, không khoét; siết gate sườn dốc (`grad>0.004 & f<0.72`).
- **Sông thon–nở theo lưu lượng**: `rad = 0.4 + s²·widthScale·2.6`, `amt0 = 90+165s` — đầu nguồn chỉ mảnh, phình rõ sau mỗi hợp lưu (dendritic đúng nghĩa; mạng hợp lưu vốn là D8 nên cấu trúc rễ cây đã có, giờ nhìn thấy được).
- **Snowline mềm** (biome #3): ngưỡng Glacier/Snow/Rock/Alpine trong `classify` nhận `capJit = (moist−0.45)·0.06` (sườn ẩm tuyết XUỐNG thấp — đúng khí tượng) và **tuyết trượt khỏi mặt dốc**: `T_SNOW − slope·0.05`, đá lộ thêm `T_ROCK + slope·0.04` → ranh giới nhấp nhô hòa quyện, vách dốc lộ sọc đá.
- **Thực vật theo nước + treeline** (biome #3): `waterBoost = 1 + shore·1.1 + min(1,flow·1.6)·0.9` nhân vào density → **rừng hành lang ven sông/hồ, ốc đảo dọc wadi sa mạc** (flora 73k→**97k**); cây cao trên `slope>0.55` tự hạ thành bụi (đất không giữ được rễ).
- **Ecotone blend** (biome #3): bake trộn màu 4 láng giềng ±2 cell khi khác biome (56/44) — nâu↔xanh chuyển tiếp mềm; nước/băng giữ mép sắc.
- **Xói mòn** (địa mạo #4): droplets n·0.02→**n·0.03** (cap 120k) — khe rãnh thủy lực cắt sườn núi thật hơn.
- **Chấm đen** (địa mạo #4) = cửa hang quá dày/to: prob 0.018, slope≥0.45, **cấm vùng tuyết** (tApprox<0.135), scale 1.5×1.1, ép sát vách hơn → 40 hang chất lượng thay vì 100 chấm.
- **Thác hết ghim xuyên**: màn đẩy ra 1.0 unit khỏi vách, vũng bọt nhỏ (0.55/0.4) + nâng 0.3 + opacity 0.38.
- **Verify:** build ✅ · 7/7 & 237/237 ✅ · lint 0 lỗi · smoke: flora 97k, 450 thác, 40 hang, 24 hồ, 22/22 biome, 3.9s, 0 NaN · screenshot: rừng bám dọc sông rõ, sông thon-nở, hết mảng nước/sọc song song. 0 lỗi console.
- **Tinh chỉnh:** độ nhấp nhô snowline → `capJit`/hệ số slope; mật độ ven sông → `waterBoost`; blend ecotone → trọng số `0.56/0.11`; xói mòn → `n*0.03`.

---

# 💧 v16 — FIX nước xuyên sườn dốc + rừng dày hơn + 450 thác (2026-07-03)

Bug user báo: mesh nước "lồi ra thành mảng tam giác/chữ nhật sắc cạnh" cắt xuyên terrain ở sườn dốc. **Bump `WORLD_GEN_VERSION` 15→16... (thực tế 14→15** trong code — flora/thác đổi).

- **Nguyên nhân clipping**: hồ/ao render bằng quad phủ **bbox** bồn ở cao độ mặt tràn; địa hình **dưới mực nước nhưng NGOÀI lòng bồn** (sườn dốc xuôi) vẫn có depth>0 → depth-fade không giấu được → mảng nước lơ lửng bị terrain cắt thành hình sắc cạnh. Ao-trên-sườn từ v13 (carve pits) nhân lỗi lên.
- **Fix**: **`uLakeMask`** — texture R8 full-res từ trường `water[]` (1 = cell ngập thật), bilinear cho mép mềm; shader hồ nhân `alpha *= smoothstep(0.2, 0.55, mask)` (chỉ khi `uMaskOn=1`; ocean giữ nguyên). Nước bám đúng lòng bồn tuyệt đối, không phụ thuộc subdivision.
- **Rừng/thực vật dày hơn**: `maxFlora` 90k→130k, density tăng toàn bộ biome có cây (Forest 0.4→0.5, Jungle 0.55→0.62, Taiga→0.52, Grassland→0.22, Mangrove→0.4, Bog→0.3, Steppe→0.12…) → **73.4k instance** @2048² (+20%).
- **Nhiều thác hơn**: `MIN_DROP` 0.012→0.009 (base 1024), `MAX_FALLS` 320→**450** (đạt cap), khoảng cách tối thiểu 4→3 cell.
- **Verify:** build ✅ · 7/7 & 237/237 ✅ · lint 0 lỗi · smoke: 450 thác, flora 73.4k, hồ 27, 0 NaN · screenshot aerial vùng sông-ao trên sườn: **hết mảng nước sắc cạnh**, thấy hoa dại + chim + thác trong khe. 0 lỗi console.
- **Tinh chỉnh:** mép mask → cặp `0.2/0.55` trong shader; mật độ rừng → `floraDensity`; số thác → `MIN_DROP`/`MAX_FALLS`.

---

# 🐦 v15 — Vân đất cận cảnh + thác chảy động + quầng trăng + chim (2026-07-03)

Yêu cầu: "đẹp và chân thực hơn nữa". **KHÔNG bump WORLD_GEN_VERSION** (chỉ render-side — cache người dùng còn nguyên).

- **Micro-detail texture** (`WorldTerrain.buildDetailTexture` + `onBeforeCompile`): tile value-noise 256² (2 octave, mean-1.0) nhân vào diffuse sau `map_fragment` với repeat ×220, cường độ 0.34 → mặt đất có hạt/vân khi đi bộ (trước bị bilinear kéo mờ trong vài mét); mean-centred nên mipmap tự trung hòa về 1.0 = tự tắt theo khoảng cách, 0 chi phí tune. Lưu ý pattern: prepend `uniform sampler2D uDetail;` vào fragmentShader + replace include; uv dùng `vMapUv` (three ≥ r151).
- **Thác chảy động** (`WorldWaterfalls`): curtain đổi sang ShaderMaterial — sọc sáng 2 tốc độ trượt xuống theo `uTime`, vỡ theo cột (hash), mép trái/phải fade mềm, chân thác trắng bọt hơn. **Bài học InstancedMesh + ShaderMaterial**: three TỰ define `USE_INSTANCING` + khai báo `instanceMatrix` — chỉ cần NHÂN nó trong transform, khai báo tay sẽ lỗi "redefinition".
- **Quầng trăng** (`WorldSky` shader): uniforms `uMoonDir/uMoonGlow` — halo lạnh `pow(m,700)·0.55 + pow(m,26)·0.05`, glow = moonDirY khi trăng mọc. Đêm có vệt trăng lấp lánh trên nước (roughness map + spec).
- **Chim** (`WorldBirds.tsx` mới): 18 con, silhouette 2 tam giác chữ V, 1 InstancedMesh + 1 draw call; bay vòng tròn (tham số deterministic per-index), nghiêng cánh theo chiều lượn, vỗ cánh bằng squash scale-Y; frustumCulled=false (matrix update mỗi frame ~20 phần tử — không đáng kể).
- **Verify:** build ✅ · 7/7 & 237/237 ✅ · lint 0 lỗi · screenshot: đất có grain, đêm 00:21 vệt trăng trên biển, curtain compile sạch, 0 lỗi console.
- **Tinh chỉnh:** độ đậm vân đất → `0.34` + repeat `220`; tốc độ sọc thác → `1.5/2.6`; độ sáng quầng trăng → hệ số `0.55/0.05`; số chim/độ cao → props `count` + hằng trong `flights`.

---

# 🌅 v14 — Trời gradient + AO địa hình + hồ băng + hoa dại + địa tầng (2026-07-03)

Yêu cầu: "đẹp và chân thực hơn nữa, làm đầy đủ map/môi trường/địa hình". **Bump `WORLD_GEN_VERSION` 13→14** (flora mix đổi).

- **Bầu trời gradient + quầng mặt trời** (`WorldSky`): vòm đổi từ 1 màu phẳng sang ShaderMaterial — zenith sẫm (skyColor×0.62), chân trời nhạt dần về màu sương (scale theo độ sáng trời nên đêm không bị bạc), halo mềm quanh mặt trời (`pow(s,300)·0.9 + pow(s,10)·0.16·sunIntensity`). Bỏ animation xoay vòm (halo cần phương vị ổn định). Mây 8→14 cụm.
- **AO độ cong bake vào color texture** (`WorldTerrain`): 2 vòng ring-sample (R≈3/9 cell @2048, world-space constant) — khe/hẻm/lòng chảo tối dần, đỉnh gờ sáng nhẹ; cap 0.22, dưới biển ×0.5 (giữ thềm turquoise nắng). Đủ mạnh để "đóng ghim" lòng sông/khe núi, không dìm cả bồn địa rộng.
- **Vân địa tầng**: sọc ngang ±5% (Badlands ±7.5%) theo `sin(e·320)` trên mặt Rock/Badlands dốc >0.45 — vách đá có lớp trầm tích thẳng hàng toàn vách.
- **Hồ đóng băng** (`WorldWater` + minimap + bake): bồn có nhiệt độ tâm <0.19 (cực hoặc núi cao nhờ lapse) → tấm băng đục `#d7e9f2` roughness 0.35 (mesh gộp riêng, +0.12 nổi trên mặt), lòng hồ bake màu băng nhạt (hết viền navy lộ mép), minimap tint trắng xanh. 3/27 bồn đóng băng ở seed hiện tại.
- **Hoa dại** (`WorldVegetation`): Tuft đổi base sang trung tính nhạt; instanceColor quyết định — 87% cỏ xanh (dải sắc), 5.5% hồng, 4.5% vàng, 3% trắng → đồng cỏ thành thảm hoa rải rác.
- **Đá tảng glacial erratic/scree**: pickFlora thêm Rock ~5-10% cho Taiga/Tundra/Alpine (1.5k tảng @2048²).
- **Verify:** build ✅ · 7/7 & 237/237 ✅ · lint 0 lỗi · smoke 22/22, 3.76s · screenshot: trời gradient+halo, thung lũng sông có chiều sâu, hồ băng xám giữa lòng chảo tuyết, 0 lỗi console.
- **Tinh chỉnh:** cường độ AO → hệ số `22/10` + cap `0.22`; ngưỡng đóng băng → `0.19` (WorldWater + Minimap + bake — giữ đồng bộ 3 nơi); tỷ lệ hoa → các mốc `0.055/0.1/0.13`; sọc địa tầng → `sin(e*320)`; màu trời → công thức zenith/horizon trong useFrame của WorldSky.

---

# 🏞 v13 — Sông suối đẹp: ribbon uốn lượn + lòng sông khoét + nước bóng (2026-07-03)

Yêu cầu: "sông suối đang trông rất tệ; nhẹ nhưng vẫn đẹp". Gốc rễ cái xấu: kênh D8 **thẳng tắp theo chéo lưới, 1 cell, song song cách đều** trên sườn dốc → mạng sọc xanh cơ học. **Bump `WORLD_GEN_VERSION` 12→13.**

## Generation (Pass 2b mới trong `worldGen.ts`)
- **Ribbon sông** (`riverAmt: Uint8Array` — field mới): stamp đĩa theo cường độ flow dọc kênh → sông **rộng dần về hạ lưu** (ramp bậc hai: chỉ trunk mới rộng, nhánh mảnh), bờ feather mềm 1 cell. Ngưỡng `CORE_T=0.615` (= ngưỡng biome River mới) nằm **trên dải rãnh-sườn-đồi** của D8.
- **Meander wiggle**: tâm stamp warp bằng noise mượt (`wig=1.6×widthScale`, freq 0.18) → mỗi kênh uốn lượn riêng, phá thế song song; kênh vẫn liền mạch vì cell kề offset gần như nhau.
- **Gate sườn dốc**: mặt rất dốc chỉ giữ ribbon khi flow ≥ 0.68 (sông lớn xuyên hẻm vẫn còn, rãnh fall-line mỏng bị loại — hết hatch).
- **Khoét lòng** −0.0035×amt vào heightmap TRƯỚC slope/biome/hồ → sông nằm trong bed thật, normal map tự đổ bóng bờ; giữ dưới `LAKE_MIN_DEPTH` để không thành hồ giả (lakeBasins 27 ✓).
- Biome River tô lại theo ribbon core (≥140, chừa Ocean/Lake/Glacier/Snow); flora né `riverAmt>100`; validator/worker/isRenderable thêm field.

## Render (`WorldTerrain.tsx`)
- Màu nước bake theo `riverAmt` gradient (tâm sẫm hơn 10%), **chừa Glacier/Snow** (hết sọc xanh vắt qua băng). Bỏ hẳn stream-tint cũ theo flow (0.44–0.6) — thủ phạm mạng nhện.
- **`roughnessMap` mới** (đọc kênh G): sông/hồ/ao 0.33, thềm ngập 0.5, cát ướt 0.7, đất 0.95 → **mặt nước bắt sáng lấp lánh theo mặt trời** (material roughness=1 × map).
- **Nhẹ hơn**: normal map + roughness bake ở **size/2** (2048→1024) → GPU texture 32MB→**24MB** dù thêm map mới; bake nhanh ~4×.
- Camera orbit: teleport nâng `target.y` theo mặt đất + **kẹp camera ≥ mặt đất+2** (hết chui gầm map thấy màu trời).

## Verify
- Smoke @2048²: 3.8s · ribbon 2.34% map (core 0.62%) · 27 hồ/ao · 181 thác · 22/22 biome · 0 NaN.
- Screenshot: aerial sông = mạng phân nhánh ao→suối→sông uốn khúc tự nhiên; walk-mode đứng bờ suối thấy ribbon lượn xuống dốc; **0 lỗi console**. Build ✅ 7/7 & 237/237 ✅ lint 0 lỗi.
- **Tinh chỉnh:** độ rộng sông → hệ số `1.8` trong `rad`; độ uốn → `wig`/freq `0.18`; ngưỡng bắt đầu → `CORE_T`; độ sâu lòng → `0.0035`; độ bóng nước → giá trị `84` trong `buildRoughnessTexture`.

---

# 🎥 v12 — Camera 5 chế độ + Thác/Suối/Ao/Hang + 11 loại thực vật + chế độ Nhẹ (2026-07-03)

Yêu cầu: "cam có các góc nhìn khác, nhiều môi trường hơn (thác, suối, ao, hang động…), nhiều thực vật hơn, nhẹ CPU/GPU". **Bump `WORLD_GEN_VERSION` 11→12.**

## Camera (`WorldCameraRig.tsx` — mới)
- **5 chế độ** (nút HUD): 🌀 Quay (orbit cũ) · 🕊 **Bay** (WASD + E/Q lên/xuống, giữ chuột trái kéo để nhìn, Shift ×3.2) · 🚶 **Đi bộ** (góc nhìn người, WASD, dính mặt đất qua `sampleMeshHeight`, lội ở mép nước, vào mode là đứng NGAY tại điểm target/teleport) · 🗺 Trên cao (khóa xoay, pan+zoom) · 🎬 Cine (tự quay quanh điểm quan tâm).
- Teleport minimap hoạt động ở MỌI chế độ; `viewRef` cập nhật từ rig cho minimap/HUD.

## Môi trường nước & hang (`worldGen.ts` v12)
- **Thác nước**: dò ô sông có drop/cell vượt ngưỡng (chuẩn hóa theo resolution) → SoA `waterfall*` (121 thác @2048²); render `WorldWaterfalls.tsx` = màn nước gradient trắng-xanh + vũng bọt chân thác, **2 draw call** cho toàn bộ.
- **Suối**: nhánh flow 0.44–0.6 tint xanh nước mảnh vào color texture (brook→stream→river liền mạch).
- **Ao**: `MIN_LAKE_CELLS` 16→5, `MAX_LAKES` 520; **toàn bộ hồ/ao gộp 1 mesh** (mergeGeometries — trước là 280 mesh/draw call). Shader nước chuyển sang **world-space swell** để phục vụ mesh gộp.
- **Hang động**: `cave*` SoA — đĩa gần đen unlit áp vào vách `Rock/Badlands` slope>0.35 (100 hang @2048², 1 draw call, `WorldCaves.tsx`); trên núi đọc như hốc đá thật.

## Thực vật 5 → 11 loại (`pickFlora` trộn theo biome)
Pine (thông 2 tầng) · Round (tán kép) · Jungle (tầng tán) · Cactus (có nhánh) · Rock · **Acacia** (dù savanna) · **Palm** (5 tàu lá — mangrove/jungle) · **DeadTree** (khô — desert/badlands/chaparral) · **Bush** (bụi kép) · **Reed** (lau sậy — đầm/bog) · **Tuft** (cỏ — grassland/steppe/alpine/tundra). Mix @2048²: Pine 20.8k, Bush 14.6k, Round 10.5k, Tuft 8.2k, Jungle 5.1k, Reed 1.6k, Palm 1.5k… Tundra giờ KHÔNG còn cây (đúng thực tế). Chỉ cây cao castShadow.

## Nhẹ CPU/GPU
- Nút **GPU: Đẹp/Nhẹ** trên HUD — áp LIVE không remount context (`QualityApplier`: setDpr 1.5→1, `gl.shadowMap.enabled` + recompile material, cây bỏ shadow + giữ 1/2 instance). Bài học: remount `<Canvas key>` làm chết render loop (màn đen) — đừng remount, toggle runtime.
- `dpr` mặc định cap [1, 1.5]; `powerPreference: high-performance`; hồ 280→1 draw call; ground-cover không đổ bóng.
- **Rộng hơn**: `RENDER_SIZE` 1000→**1200**.
- Cát ướt: dải beach sát mép nước tối đi 14% (swash zone) trong color bake.

## Verify
- Smoke @2048²: 4.1s · 22/22 biome · 121 thác · 100 hang · flora 63k/11 loại · 0 NaN. @1024²: 1.0s.
- Playwright: default/walk/fly/cine/low đều sạch, **0 lỗi console**; walk đứng đúng điểm teleport nhìn ra hồ+rừng+suối; fly thấy thác trắng trên vách; Nhẹ render tức thì.
- Build ✅ · test 7/7 & 237/237 ✅ · lint 0 lỗi (432 warning legacy-any, +3 từ pattern `args as any` có sẵn).

## Tinh chỉnh nhanh
- Ngưỡng thác → `MIN_DROP` (Pass 4e); số/kích thước hang → prob `0.02`/`slope>0.35` (Pass 4f) + scale trong WorldCaves; độ đậm suối → hệ số `0.55` trong buildColorTexture; mật độ từng loài → `floraDensity`/`pickFlora`; tốc độ bay/đi bộ → `base` trong WorldCameraRig; ngưỡng ao → `MIN_LAKE_CELLS`.

---

# 🧹 BACKEND — clippy sạch (lib) + rustfmt toàn crate (2026-07-03)

- `cargo clippy --fix` + sửa tay 4 `needless_range_loop` → **lib 0 warning**. `cargo fmt` chuẩn hóa toàn bộ crate (trước đây chỉ format từng file qua hook → diff lớn 1 lần, sạch về sau). Toàn bộ `cargo test` pass.
- Còn lại (chỉ trong test binaries, không đụng semantics): `unused Result` ×8, `MutexGuard across await` ×5 (adversarial_challenger), `clamp-like` ×3, dead-code TrackingAllocator — sửa sau nếu muốn clippy --all-targets sạch 100%.

---

# 🌏 KHÍ HẬU THẬT v11 — Rain shadow + 22 biome vùng lớn + texture full-res (2026-07-03)

Yêu cầu: "map chân thật, tối ưu, đẹp, nhiều môi trường nhất có thể". **Bump `WORLD_GEN_VERSION` 10→11** (cache tự sinh lại). Đã commit cùng ngày.

## Generation (`worldGen.ts`)
- **Khí hậu kiểu Trái Đất**: quét **bóng mưa (rain shadow)** 2 chiều gió + đai gió theo vĩ độ (mậu dịch đông→tây ở nhiệt đới, gió tây ôn đới, gió đông vùng cực); **đai khô Hadley** (~lat 0.62) bảo đảm sa mạc đúng chỗ trên mọi seed + **ITCZ** giữ rừng mưa xích đạo. Tốc độ khô/ẩm chuẩn hóa theo world-distance (không phụ thuộc resolution — 2048 không còn khô gấp đôi 1024).
- **Đường tuyết/đá theo nhiệt độ ĐÃ TRỪ LAPSE** (vĩ độ + độ cao, lapse 1.35): băng vĩnh cửu ở cực sát mực biển, đỉnh núi xích đạo vẫn đóng tuyết. Ngưỡng `T_GLACIER/T_SNOW/T_ROCK/T_ALPINE` trong `classify`.
- **Ma trận Whittaker đầy đủ → 22/22 biome hiện diện** thành VÙNG LỚN liền mạch (trường khí hậu mượt ở tầm lục địa); Badlands = khô + dốc; Mangrove bờ nóng-ẩm thoải; Bog (lạnh) / Swamp (ấm); Savanna đổi flora sang cây tán tròn (keo), chỉ Desert còn xương rồng.
- **Lọc đa số 3×3** khử speckle phân loại (nước/sông/bãi/mangrove/băng không tham gia — chúng mỏng hợp lệ).
- **seaLevel = phân vị histogram của elevation SAU erosion** → đất đúng `LAND_FRACTION` 38% với mọi seed; `FREQ` 1.35→0.95 → MỘT lục địa khổng lồ + đảo vệ tinh (hết quần đảo vụn).
- **Tối ưu**: counting sort 16-bit cho flow pass (O(n), thay sort so sánh 4M index) → **2048²: ~3.8s** (v10 ~5.0s), 1024²: ~1.0s, 0 NaN.

## Render
- `WorldTerrain`: màu biome bake vào **DataTexture sRGB full-res** (2048² ~16MB GPU; mesh 384 chỉ lo hình khối) + jitter sáng ±4.5%/cell + mipmap + anisotropy 8 → biên giới biome/sông/bãi sắc nét ở mọi khoảng cách, GPU tự blend chuyển tiếp; **normal map PHẦN DƯ** (elevation full-res − mặt mesh bilinear) thêm chi tiết xói mòn/gờ núi mà không double-shading với normal đỉnh. Lưu ý: bake texture ~0.5–1s main-thread 1 lần khi load (tương lai: chuyển vào worker).
- `WorldWater`: **fog thủ công đồng bộ `scene.fog` mỗi frame** (nước tan vào sương y như đất — hết vệt sáng/mép cứng chân trời); mọi chi tiết (sóng vertex, micro-normal, specular, foam) **tắt dần theo khoảng cách** (hết moiré + cột glitter aliasing); **fix lỗ thủng biên map**: ngoài footprint heightmap coi đáy = biển sâu (trước đây clamp UV lấy chiều cao ĐẤT ở hàng biên → depth 0 → alpha 0 → "quạt trời" xuyên biển tới vô cực); **foam theo độ sâu tuyệt đối 1.5 unit** (hết foam phủ kín hồ núi nông); Fresnel phản chiếu màu sương trời (không phải turquoise nông); plane biển ×30 renderSize + `frustumCulled=false` (mép vượt far-clip từ mọi vị trí orbit).
- `WorldVegetation`: cây ghép **2 tông trong 1 geometry** (thân nâu + tán lá, `mergeGeometries` + vertex color — vẫn 1 InstancedMesh/loại), tán kép, thông 2 tầng, xương rồng có nhánh; **instanceColor** biến thể sáng ±18% + lệch tông/cây (rừng hết cảm giác "1 cây nhân bản"); `castShadow` (trừ đá).
- `WorldSky`: mây = cụm ellipsoid dẹt low-poly (hết slab/cột — hệ số cũ ×4 tạo blob 400 unit chiếm nửa trời), cao hơn (~1080) + tản rộng ±1350.
- `WorldShowcase`: hook chẩn đoán `window.__worldScene` (tooling bật/tắt object khi debug render).

## Verify
- Smoke node (esbuild bundle **`world_smoke.ts` ở root — GIỮ LẠI để tune tiếp**): 22/22 biome @1024² & @2048², 0 NaN, đất 38.0%, phân bố lành mạnh (Steppe 15% · Taiga 13% · Grassland 12.5% · Shrubland 11% · Glacier 10% · … · Desert 0.85% · Badlands 0.33% đất). Chạy: `npx esbuild world_smoke.ts --bundle --platform=node --format=cjs --outfile=<tmp>/s.cjs && node <tmp>/s.cjs` (env `SMOKE_SIZE`, `SMOKE_OUT`).
- `world_preview.png` (root, tracked) đã tái sinh @2048² — đúng world app render (seed `seed`).
- Screenshot Playwright headless (SwiftShader, flags `--enable-unsafe-swiftshader`) trên `landscape.html`: default + teleport-zoom sạch — hết quạt trời/moiré/foam hồ; cần Pause ngay khi world ready vì đồng hồ ngày/đêm chạy theo wall-time.
- `npm run build` ✅ · `npm run test` 7/7 ✅ · `npm run test:frontend` 237/237 ✅ · lint 0 lỗi (429 warning cũ).

## Tinh chỉnh nhanh
- Tỷ lệ đất → `LAND_FRACTION`; độ lớn lục địa → `FREQ`; đai khô → biên độ `0.16`/tâm `0.62` của `hadleyDry`; cường độ bóng mưa → `RISE/DRIZZLE/LAND_ET/OCEAN_WET`; ngưỡng biome → ma trận trong `classify()`; đường tuyết → `T_*`; jitter màu đất → `0.95 + hash*0.09` trong `buildColorTexture`; độ sâu foam → hằng `1.5` trong WorldWater; kích thước/độ cao mây → hệ số trong `clouds` useMemo của WorldSky.

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
- **Tinh chỉnh:** độ nhạy slope → hệ số `0.06` + `step` trong `computeSlope`; ngưỡng vách đá → `slope>0.85` trong `classify`; mật độ cây → `floraDensity` × `(0.5+moisture)`.

---

# 🎨 FIX 3D vs MINIMAP — Màu terrain & luật trồng cây (2026-07-01)

Terrain 3D bị "xám/trắng nhợt nhạt loang lổ", không khớp minimap; cây mọc dưới nước/ven hồ/trên đá. **Bump `WORLD_GEN_VERSION` 5→6.**
1. **Màu 3D = màu minimap.** Nguyên nhân: `WorldTerrain` chồng 3 lớp tint (cát theo `shore`, đá xám theo `slope`, nước theo `flow`) lên màu biome → ở map 2048² bờ biển/hồ rất dài nên tint cát+đá phủ phần lớn đất, thành xám nhạt loang lổ. **Fix:** bỏ hết tint chồng, màu vertex = `BIOME_RGB[biome]` **thuần** — đúng bảng màu minimap dùng (sông/bãi cát/hồ vốn đã là biome River/Beach/Lake nên vẫn hiện). Đổ bóng địa hình do ánh sáng scene lo, không tint vào màu.
2. **Luật trồng cây chặt** (`worldGen` flora pass): chỉ mọc khi `elevation > seaLevel` **và** `water==0` **và** `slope ≤ 0.78` **và** `shore ≤ 0.8` (không sát mép nước) **và** biome hợp lệ. `floraForBiome` giờ trả `-1` cho Ocean/Beach/River/Lake/Snow/Glacier/**Rock/Badlands** (bỏ đá/boulder trên núi trọc). Validate lại **ô sau khi jitter** để cây không lệch ra nước. Mật độ vẫn ×(0.5+moisture).
- **Verify:** build ✅ · 237/237 ✅ · lint 0 lỗi. Smoke @1024²: flora=67.6k, **0 cây dưới nước / trên bãi/tuyết/đá / dưới mực nước / vách dốc**.

---

# 🏖 BÃI BIỂN — dải cát ven bờ (2026-07-01)

Biome `Beach` đã có nhưng dải quá hẹp (`seaLevel+0.022`) nên gần như không thấy. **Bump `WORLD_GEN_VERSION` 6→7.**
- **Nới dải cát**: đất có `elevation ∈ [seaLevel, seaLevel + 0.05)` → luôn là `Biome.Beach` (màu cát `#EAD8A2`). `WorldTerrain` tô thẳng `BIOME_RGB[Beach]` nên bãi cát **khớp giữa minimap và 3D**.
- **Không cây/đá trên cát**: `floraForBiome(Beach) = -1` + gate `shore≤0.8` + validate ô jitter → bãi cát nhẵn hoàn toàn.
- Mangrove lùi vào **trong** bãi cát (`elev < beach+0.04 && hot && very-wet`) để không đè lên dải cát.
- **Verify:** build ✅ · 237/237 ✅ · lint 0 lỗi. Smoke @1024²: 97.6k ô cát, **97.6% dải ven bờ là Beach**, **0 flora trên cát**.
- **Tinh chỉnh:** độ rộng bãi → `BEACH_WIDTH` trong `classify`; màu cát → `BIOME_RGB[Beach]`.

---

# 🏝 BÃI BIỂN v2 — Làm phẳng ven bờ + Beach theo độ dốc (2026-07-01)

Lỗi: cát dính lên vách đá dốc, bãi hẹp/dốc. **Bump `WORLD_GEN_VERSION` 7→8.**
1. **Làm phẳng ven biển (Pass 1c, heightmap)**: ô có `elevation ∈ [seaLevel, seaLevel+0.14]` được kéo về **ramp thoải** (`flat = seaLevel + 0.35·(e−seaLevel)`) rồi blend ngược về địa hình thật ở trên bằng `smoothstep` → bờ dâng rất từ từ từ mặt nước. Nhiều ô rơi vào dải cát hơn → **bãi rộng & thoai thoải**; mũi đất dốc vẫn dâng nhanh nên vẫn là vách đá.
2. **Beach theo slope (`classify`)**: ô ven biển **CHỈ** là `Beach` khi `slope ≤ 0.3` (bằng phẳng); nếu dốc (vách đá đâm xuống biển) → `Rock`. Mangrove cũng chỉ mọc ở bờ thoải.
- **Frontend không đổi**: `WorldTerrain` vẫn tô `BIOME_RGB[biome]` → cát/vách đá do biome quyết định (theo slope) tự hiện đúng và **khớp minimap** (không lặp tint gây xám).
- **Verify:** build ✅ · 237/237 ✅ · lint 0 lỗi. Smoke @1024²: beach=92k (rộng), **maxBeachSlope=0.30 (cát không bao giờ dốc)**, coastalRock=59k (vách đá ven biển), **0 flora trên cát**, 0 NaN, elevation∈[0,1].
- **Tinh chỉnh:** độ rộng/thoải bãi → `COAST_BAND`/`COAST_GAIN` (Pass 1c); ngưỡng cát↔vách → `BEACH_MAX_SLOPE` trong `classify`.

---

# 🌊 BỜ BIỂN VẬT LÝ v3 — Thềm 2 bên mực nước + đáy cát nông (turquoise) (2026-07-01)

Làm đúng 3 nguyên lý vật lý của coastline. **Bump `WORLD_GEN_VERSION` 8→9.**
1. **Thềm lục địa đối xứng (Pass 1c)**: terrace giờ làm phẳng **CẢ hai bên** mực nước — ramp thoải phía trên (`COAST_BAND=0.14`) **và thềm nông phía dưới** (`SHELF_BAND=0.09`), blend về địa hình thật bằng `smoothstep`. Bờ dâng từ từ từ mặt nước và **lài dần xuống đáy nông**, tạo chỗ cho bãi rộng.
2. **Cát theo độ dốc (`classify`)**: ô thuộc thềm ven bờ `[seaLevel−SHALLOW, seaLevel+BEACH_WIDTH]` → **Beach nếu `slope ≤ 0.3`**, **Rock nếu dốc** (vách đá — dù nhô lên hay đâm xuống nước). Cát không dính trên vách.
3. **Đáy cát nước nông**: ô vừa chìm dưới nước `[seaLevel−0.06, seaLevel)` mà thoải → **Beach (đáy cát)**. Qua Water Shader trong suốt → **ánh xanh ngọc lam (turquoise)** ở nông rồi chuyển **xanh thẫm** ở sâu. Shader: nông trong hơn (`alpha=mix(0.4,0.92,depthN)`) + màu nông turquoise `#48DDCA` → đáy cát hắt lên.
- **Verify:** build ✅ · 237/237 ✅ · lint 0 lỗi. Smoke @1024²: bãi khô 98k + **đáy cát nông 87k** + vách đá ven biển 110k, **0 ô cát dốc**, **0 flora trên cát**, 0 NaN.
- **Tinh chỉnh:** độ sâu thềm nông → `SHELF_BAND`; bề rộng đáy cát → `SHALLOW`; sắc turquoise/độ trong → `uShallow`/`alpha` trong WorldWater.

---

# 🗺 ĐẠI TU TERRAIN — Bản đồ sạch: lục địa lớn, đồng bằng phẳng, biome theo tầng cao (2026-07-02)

Terrain trước bị "lốm đốm như bùn" + cát phủ khắp đảo. Đại tu theo 3 nguyên lý. **Bump `WORLD_GEN_VERSION` 9→10.** (Ánh sáng scene cũng đã hạ ở commit trước để màu không cháy trắng.)
1. **Noise (cấu trúc hình học)**: hạ base `FREQ 2.4→1.35` (lục địa/đại dương lớn), giảm gain fBm `0.5→0.42` + **bỏ lớp roughness `*6`** (hết vụn), `pow(e) 1.25→1.7` (đồng bằng phẳng rộng), **giảm erosion** `droplets n*0.06→n*0.02` (bớt nhấp nhô). Bỏ hẳn `Pass 1c` coastal flattening (thủ phạm tạo cát tràn).
2. **Đồng bộ minimap**: cả minimap và 3D **đã dùng chung `BIOME_RGB[biome]`** — chỉ cần phân loại biome sạch là khớp. Không còn tô 2 kiểu.
3. **Biome theo tầng độ cao (strict)**: `< seaLevel` Ocean → `+0.02` **cát mỏng** (gate thêm bằng **khoảng cách tới biển `coast`** để không tràn ra đồng bằng phẳng; dốc→Rock cliff) → tới `ROCK_LEVEL=0.66` là **phần xanh chủ đạo** (Grassland/Forest/Jungle/Taiga/Tundra/Swamp theo khí hậu) → `0.66–0.86` **Rock** → `>0.86` **Snow**. Bỏ các biome tông vàng (Desert/Savanna/Steppe/Chaparral/Badlands…) khỏi đất thường để hết "sandy".
- **Verify (smoke @1024²):** **green 75%** (chủ đạo) · **beach 5%** (mỏng) · rock 14% · snow 3% · **rough 5.4e-4** (mượt, hết lốm đốm) · 0 NaN. Build ✅ · 237/237 ✅ · lint 0 lỗi. Screenshot in-browser: đảo xanh mướt + bãi cát mảnh + núi tuyết, **khớp minimap**.
- **Tinh chỉnh:** kích thước lục địa → `FREQ`; độ phẳng đồng bằng → số mũ `pow`; ngưỡng tầng → `ROCK_LEVEL`/`SNOW_LEVEL`; độ mỏng bãi → `BEACH_TOP` + ngưỡng `coast>0.65`.

## Cách chạy / kiểm tra nhanh
- Dev: `npm run dev` → mở `http://localhost:5173/landscape.html` (lần đầu sinh ~1s off-thread, sau đó cache → tức thì).
- Sinh thế giới mới: gọi `clearWorldCache()` hoặc bump `WORLD_GEN_VERSION` trong `worldGen.ts`.
- Tinh chỉnh: núi cao/thấp → `HEIGHT_RATIO`; to/nhỏ → `RENDER_SIZE`; chi tiết mesh → `MESH_RES`; nhiều desert hơn → ngưỡng trong `classify()` của `worldGen.ts`.
- Ảnh preview tĩnh (biome+hillshade) đã sinh: `world_preview.png` ở thư mục gốc.

## Lưu ý
- Các bước trên đã được commit (xem git log quanh 2026-07-01→03).
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
