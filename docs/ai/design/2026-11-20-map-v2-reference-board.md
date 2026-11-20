---
phase: design
feature: map-v2
title: Design — Bảng tham chiếu G-REF của Map v2 (53 ảnh CC0/public domain)
description: Bộ ảnh phong cảnh thật đã được chủ dự án duyệt ngày 2026-11-20 làm chuẩn cổng G-REF của Map v2. Có 53 ảnh CC0 hoặc public domain thuộc 13 môi trường; mỗi ảnh có nguồn, giấy phép đã kiểm, tiêu chí thị giác và mốc liên quan. Ảnh không nằm trong repo; danh sách máy đọc được nằm ở artifacts/map-v2/reference-board.json
status: accepted
owner: simulation-architecture
last_reviewed: 2026-11-20
---

# Bảng tham chiếu G-REF của Map v2

> **Chủ dự án đã duyệt ngày 2026-11-20 ("Duyệt").** Đây là chuẩn của cổng G-REF trong
> [kế hoạch Map v2](../requirements/2026-11-20-feature-map-v2.md) (pha P0). Mọi ảnh chụp chuẩn của bản đồ ở các mốc
> R1–R4 và P1 được đặt cạnh các ảnh này. Ảnh không được commit; mỗi mục có trang nguồn và giấy phép đã kiểm.
> Danh sách máy đọc được: [`artifacts/map-v2/reference-board.json`](../../../artifacts/map-v2/reference-board.json).

Bảng này dùng cho cổng **G-REF** của Anima-Engine Map v2: mỗi biome của bản đồ được đặt cạnh ảnh thật, chủ dự án duyệt bằng mắt. Nhóm 12 (ảnh vệ tinh, mạng thoát nước) cũng là đầu vào cho cổng **G-SHAPE**. Dữ liệu máy đọc nằm ở `refs.json` cùng thư mục.

Lập ngày 2026-11-20. Tổng **53 ảnh** trong 13 nhóm.

## Quy tắc giấy phép và cách kiểm

- Chỉ nhận **CC0** hoặc **public domain**: ảnh của NASA, USGS, NOAA và các cơ quan liên bang Mỹ khác (NPS, USFWS, BLM, USFS, USDA), file Wikimedia Commons gắn nhãn PD hoặc CC0, Poly Haven (toàn bộ CC0). Không nhận CC BY, CC BY-SA, giấy phép Unsplash, Pexels, hoặc bất cứ thứ gì lẫn lộn hay không rõ.
- Mỗi ảnh: đã mở trang nguồn và đọc hộp giấy phép trên chính trang đó; với Commons còn quét toàn bộ phần nội dung trang để bắt chữ CC BY, GFDL, ShareAlike, NonCommercial, Unsplash. Ba file đã qua vòng tìm kiếm nhưng bị loại ở bước này, cộng các file loại ngay từ kết quả tìm kiếm (xem cuối tài liệu).
- **Không tải file ảnh nào.** URL xem trước lấy từ API (Commons `iiurlwidth=1280`, Poly Haven) hoặc từ chính trang nguồn; dung lượng lấy từ header `Content-Length` của yêu cầu HTTP HEAD.
- Ảnh Commons nhỏ hơn 1280 px dùng luôn file gốc (không phóng to). Xem trước của Poly Haven là ảnh thu nhỏ 630 px; bản 760 px chiều cao (khoảng 1520 px ngang) ghi ở `larger_preview_url` trong JSON.

**Lưu ý khi duyệt:**
- Vì không tải ảnh, các tiêu chí thị giác được viết từ chú thích và metadata của nguồn cộng với kiến thức về biome. Chúng là giả thuyết cần chủ dự án xác nhận bằng mắt tại G-REF; tiêu chí nào ảnh không thể hiện thì gạch đi.
- Ảnh Poly Haven là **HDRI toàn cảnh 360° (equirectangular)**: chỉ so phần dải chân trời, hai cực trên dưới bị kéo giãn.
- Phần "bản đồ hiện đang trượt" dựa trên mô tả hiện trạng được giao (mảng màu phẳng theo biome với mép cứng, cây low-poly giống nhau, nước là mặt phẳng, không có mù, núi nhọn, sông là dải sơn), không phải do tôi chạy app đo lại.
- Tiêu chí trong từng ảnh giữ tiếng Anh để khớp từng chữ với `refs.json` (danh sách kiểm cho người làm renderer).

Mã milestone: **P1** World v2 (địa hình, thủy văn, khí hậu) · **R1** vật liệu địa hình · **R2** nước · **R3** khí quyển/ánh sáng · **R4** thực vật.

## Tổng hợp

### Số ảnh theo nhóm

| Nhóm | Số ảnh | Đạt tối thiểu 3? |
|---|---:|---|
| 1. Desert (sand dunes, rocky desert) | 4 | có |
| 2. Savanna | 3 | có |
| 3. Tropical rainforest (from above and canopy level) | 3 | có |
| 4. Temperate deciduous / mixed forest | 4 | có |
| 5. Taiga / boreal forest | 3 | có |
| 6. Tundra | 3 | có |
| 7. Swamp, marsh, mangrove | 4 | có |
| 8. High mountains, snow, glaciers (treeline, snowline) | 6 | có |
| 9. Coast and beach (rocky coast, surf, foam) | 4 | có |
| 10. River valleys and waterfalls | 4 | có |
| 11. Lakes in a landscape | 3 | có |
| 12. Aerial/satellite drainage (G-SHAPE) | 8 | có |
| 13. Atmosphere / aerial perspective / golden hour (optional) | 4 | có |
| **Tổng** | **53** | |

### Theo giấy phép

| Giấy phép | Số ảnh | Dung lượng xem trước |
|---|---:|---:|
| CC0 | 21 | 9.35 MB |
| Public domain | 32 | 12.78 MB |

Chi tiết public domain theo chủ thể: NASA 9, NPS 9, USFWS 6, BLM 3, USFS 1, USGS 1, NOAA 1, USDA 1; cá nhân tự đặt vào public domain 1 (MTN-02). CC0 gồm 14 ảnh Commons của cá nhân và 7 HDRI Poly Haven.

### Theo nguồn

| Nguồn | Số ảnh | Dung lượng xem trước |
|---|---:|---:|
| Wikimedia Commons | 45 | 19.06 MB |
| Poly Haven | 7 | 2.84 MB |
| NASA Earth Observatory | 1 | 231 KB |
| **Tổng** | **53** | **22.13 MB** |

**Ước tính tổng dung lượng tải toàn bộ ảnh xem trước: khoảng 22.13 MB** (22,129,333 byte, cộng từ `Content-Length`). Nếu dùng bản Poly Haven lớn hơn (760 px cao) thay cho bản 630 px thì cộng thêm khoảng 8.3 MB. Hai file nặng nhất là RNF-02 (PNG, khoảng 2 MB) và RNF-01 (khoảng 1,3 MB).

ambientCG không được dùng: thư viện này là vật liệu PBR chứ không phải ảnh phong cảnh, nên không hợp với việc so biome cạnh nhau.

## 1. Desert (sand dunes, rocky desert)

**Bản đồ Anima hiện đang trượt ở đâu:**

- Sa mạc là một mảng màu cát phẳng: không có đụn cát, không có sự bất đối xứng sườn đón gió / sườn trượt (DES-01, DES-02).
- Ranh giới sa mạc với cỏ/bụi là một đường cứng; thực tế là một dải chuyển tiếp bụi thưa (DES-02, DES-04).
- Không tách vật liệu theo độ dốc: cát phải đọng ở chỗ trũng, đá trần chỉ ở mặt dốc (DES-03).
- Tông cát không đổi theo hướng sườn và ánh sáng (DES-01).

### DES-01 · Sand dunes of Khongoryn Els, Gobi Desert, Mongolia

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Khongoryn_Els_10.jpg) · Xem trước: [1280×811, 96 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/e/e0/Khongoryn_Els_10.jpg/1280px-Khongoryn_Els_10.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Bernard Gagnon (own work)
- Giấy phép: **CC0** (Creative Commons Zero, Public Domain Dedication). Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): CC0 1.0 Universal Public Domain Dedication
- Milestone: P1, R1, R3
- Tiêu chí thị giác:
  - dune crests form continuous sharp ridgelines: sunlit windward slope vs darker slip face, never flat-coloured dune blobs
  - crest spacing is regular and crests share one dominant orientation (one wind regime)
  - sand tone changes smoothly with slope aspect/lighting, not per-tile colour patches

### DES-02 · Utah dunes landscape (Little Sahara Recreation Area), BLM West Desert District

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Utah_Dunes_Landscape_-_West_Desert_District.jpg) · Xem trước: [1280×841, 263 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/c/cb/Utah_Dunes_Landscape_-_West_Desert_District.jpg/1280px-Utah_Dunes_Landscape_-_West_Desert_District.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Bob Wick / Bureau of Land Management - Utah
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a Bureau of Land Management employee (US federal government work); Flickr source reviewed by FlickreviewR: Public Domain Mark
- Milestone: P1, R1, R4
- Tiêu chí thị giác:
  - lee (slip) faces are steep (~30-34 deg, angle of repose) while stoss slopes are gentle; the asymmetry is visible
  - dune field edge breaks up into scattered scrub over a transition band, not a hard biome line
  - shadowed lee faces are darker and cooler in hue than sunlit faces
- Ghi chú: Commons Featured picture.

### DES-03 · Desert in Wadi Rum, Jordan

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Wadi_rum_desert.jpg) · Xem trước: [1280×960, 354 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/c/c2/Wadi_rum_desert.jpg/1280px-Wadi_rum_desert.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Peter Chisholm (own work)
- Giấy phép: **CC0** (Creative Commons Zero, Public Domain Dedication). Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): CC0 1.0 Universal Public Domain Dedication
- Milestone: P1, R1
- Tiêu chí thị giác:
  - rock massifs rise abruptly from flat sand floors: sharp break of slope at the foot, not a smooth cone
  - sand collects on valley floors and low ground; bare rock only on steep faces
  - reddish sand and darker rock are distinct materials blending at the base, not one desert colour

### DES-04 · Goegap (HDRI, Namaqualand, South Africa) - clear midday desert, rocky outcrops

- Nguồn: [Poly Haven](https://polyhaven.com/a/goegap) · Xem trước: [630 px ngang, 336 KB](https://cdn.polyhaven.com/asset_img/thumbs/goegap.png?width=630&v=fc1866ea)
- Tác giả / ghi công: Greg Zaal / Poly Haven
- Giấy phép: **CC0** (CC0 1.0 Universal - public domain dedication, no attribution required). Bằng chứng: Asset page (opened 2026-11-20) declares licence https://creativecommons.org/publicdomain/zero/1.0/ ('CC0 1.0 Universal - public domain dedication, no attribution required'); https://polyhaven.com/license states all Poly Haven assets are CC0.
- Milestone: R1, R3, R4
- Tiêu chí thị giác:
  - sparse shrubs spaced apart with bare ground showing between them (low cover), not a uniform carpet
  - ground mixes sand, gravel and rock at metre scale
  - hard midday shadows; sky deep blue at zenith, paler toward the horizon
- Ghi chú: 360-degree equirectangular HDRI panorama: compare the horizon band, not the stretched poles.

## 2. Savanna

**Bản đồ Anima hiện đang trượt ở đâu:**

- Cây là các bản sao low-poly giống hệt, mật độ đều; thực tế là tán dẹt, cô lập, có cụm và khoảng trống (SAV-02, SAV-03).
- Cỏ một màu phẳng; thực tế đổi vàng-xanh ở thang vài mét, mùa khô ngả nâu vàng (SAV-01, SAV-02).
- Không có nắng xiên ấm, bóng dài và lớp mù xa (SAV-01).

### SAV-01 · Mpumalanga veld (HDRI, South Africa) - low sun over windswept grass

- Nguồn: [Poly Haven](https://polyhaven.com/a/mpumalanga_veld) · Xem trước: [630 px ngang, 357 KB](https://cdn.polyhaven.com/asset_img/thumbs/mpumalanga_veld.png?width=630&v=c826ee8b)
- Tác giả / ghi công: Greg Zaal / Poly Haven
- Giấy phép: **CC0** (CC0 1.0 Universal - public domain dedication, no attribution required). Bằng chứng: Asset page (opened 2026-11-20) declares licence https://creativecommons.org/publicdomain/zero/1.0/ ('CC0 1.0 Universal - public domain dedication, no attribution required'); https://polyhaven.com/license states all Poly Haven assets are CC0.
- Milestone: R4, R3
- Tiêu chí thị giác:
  - continuous grass layer whose colour varies golden-to-green at metre scale (no single flat grass colour)
  - low sun gives long shadows and warm rim light on grass tips
  - soft haze lowers contrast of distant hills
- Ghi chú: 360-degree equirectangular HDRI panorama.

### SAV-02 · Safari sunset (HDRI) - African veld, dry grass with sparse bushes

- Nguồn: [Poly Haven](https://polyhaven.com/a/safari_sunset) · Xem trước: [630 px ngang, 413 KB](https://cdn.polyhaven.com/asset_img/thumbs/safari_sunset.png?width=630&v=2aeec508)
- Tác giả / ghi công: Dimitrios Savva (photography), Jarod Guest (processing) / Poly Haven
- Giấy phép: **CC0** (CC0 1.0 Universal - public domain dedication, no attribution required). Bằng chứng: Asset page (opened 2026-11-20) declares licence https://creativecommons.org/publicdomain/zero/1.0/ ('CC0 1.0 Universal - public domain dedication, no attribution required'); https://polyhaven.com/license states all Poly Haven assets are CC0.
- Milestone: R4, R3
- Tiêu chí thị giác:
  - bushes and trees are isolated and irregularly spaced with open grass between (no grid, no forest)
  - dry-season grass is muted yellow-brown, not saturated green
  - sky gradient runs warm at the horizon to blue at the zenith
- Ghi chú: 360-degree equirectangular HDRI panorama.

### SAV-03 · Dikhololo sunset (HDRI) - veld, trees and bushes at sunset

- Nguồn: [Poly Haven](https://polyhaven.com/a/dikhololo_sunset) · Xem trước: [630 px ngang, 330 KB](https://cdn.polyhaven.com/asset_img/thumbs/dikhololo_sunset.png?width=630&v=f7620bdd)
- Tác giả / ghi công: Greg Zaal / Poly Haven
- Giấy phép: **CC0** (CC0 1.0 Universal - public domain dedication, no attribution required). Bằng chứng: Asset page (opened 2026-11-20) declares licence https://creativecommons.org/publicdomain/zero/1.0/ ('CC0 1.0 Universal - public domain dedication, no attribution required'); https://polyhaven.com/license states all Poly Haven assets are CC0.
- Milestone: R4, R3
- Tiêu chí thị giác:
  - tree density varies across the view (clumps and gaps), not uniform spacing
  - tree crowns are irregular, flattened silhouettes of varied size
  - low warm light: lit sides warm, shadows cool
- Ghi chú: 360-degree equirectangular HDRI panorama.

## 3. Tropical rainforest (from above and canopy level)

**Bản đồ Anima hiện đang trượt ở đâu:**

- Tán rừng là một mảng xanh phẳng hoặc cây lặp lại; thực tế là khảm tán lồi lõm, nhiều sắc xanh, có cây vượt tán (RNF-02, RNF-03).
- Không có phân tầng theo chiều đứng khi nhìn ở độ cao tán (RNF-03).
- Sông là dải sơn lên mặt: không mở khe trong tán, độ rộng không tăng về hạ lưu (RNF-01, RNF-02).
- Không có hơi ẩm: mây tích nhỏ, sương thung lũng, mù xa (RNF-01, ATM-02).

### RNF-01 · Afternoon 'popcorn' clouds over the Amazon rainforest (MODIS)

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Afternoon_Clouds_over_the_Amazon_Rainforest.jpg) · Xem trước: [1280×1646, 1.27 MB](https://thumb.wikimedia.org/wikipedia/commons/thumb/e/e5/Afternoon_Clouds_over_the_Amazon_Rainforest.jpg/1280px-Afternoon_Clouds_over_the_Amazon_Rainforest.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: NASA image courtesy Jeff Schmaltz, MODIS Rapid Response, NASA GSFC (NASA Earth Observatory)
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: solely created by NASA (NASA material not protected by copyright unless noted)
- Milestone: P1, R3, R2
- Tiêu chí thị giác:
  - from orbit intact forest reads as near-uniform dark green; the texture comes from clouds, not from individual trees
  - a field of small cumulus forms over forest but leaves cloud-free lanes over large rivers
  - rivers are meandering ribbons whose width grows downstream
- Ghi chú: Satellite view (from above).

### RNF-02 · Aerial view of Unuamen tropical rainforest and the Ovia-Osse River, Nigeria

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Aerial_View_of_Unuamen_tropical_rainforest_and_the_Ovia_-_Osse_River.png) · Xem trước: [1280×720, 1.98 MB](https://thumb.wikimedia.org/wikipedia/commons/thumb/a/a5/Aerial_View_of_Unuamen_tropical_rainforest_and_the_Ovia_-_Osse_River.png/1280px-Aerial_View_of_Unuamen_tropical_rainforest_and_the_Ovia_-_Osse_River.png?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Evelynkins (own work)
- Giấy phép: **CC0** (Creative Commons Zero, Public Domain Dedication). Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): CC0 1.0 Universal Public Domain Dedication
- Milestone: R4, R2, R3
- Tiêu chí thị giác:
  - closed canopy is a lumpy mosaic of crowns of different sizes and greens; no repeated identical tree
  - the river opens a gap in the canopy and bank vegetation differs from the interior
  - saturation drops with distance (aerial haze)
- Ghi chú: Oblique aerial view (from above). PNG preview is heavy (~2 MB).

### RNF-03 · Multi-stratified canopy of the forest of Dzanga-Ndoki National Park, Central African Republic

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Dzanga-Ndoki_National_Park_(10.3897-zookeys.951.53814)_Figure_27.jpg) · Xem trước: [1280×960, 527 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/c/c9/Dzanga-Ndoki_National_Park_%2810.3897-zookeys.951.53814%29_Figure_27.jpg/1280px-Dzanga-Ndoki_National_Park_%2810.3897-zookeys.951.53814%29_Figure_27.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Photo P. Annoyer, in Massa B (2020) ZooKeys 951: 47-65
- Giấy phép: **CC0** (Creative Commons Zero, Public Domain Dedication). Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): CC0 1.0 Universal Public Domain Dedication
- Milestone: R4
- Tiêu chí thị giác:
  - several vertical strata are visible: emergent crowns above a continuous main canopy, understorey below
  - emergent crowns are broad and stand clear above the canopy surface
  - no ground visible; deep shadow gaps between crowns
- Ghi chú: Canopy-level view. Small original (1512x1134); the ZooKeys article page also declares CC0 (dc.rights).

## 4. Temperate deciduous / mixed forest

**Bản đồ Anima hiện đang trượt ở đâu:**

- Một màu phẳng cho cả biome; thực tế màu trộn theo từng tán (nhất là mùa thu) và sáng tối theo hướng sườn (TMP-01, TMP-03).
- Cây nhân bản đồng loạt, nền rừng trống; thực tế thân to nhỏ khác nhau và có tầng cây bụi, dương xỉ (TMP-02).
- Bìa rừng thẳng và cứng; thực tế lởm chởm, có cây bụi và cây đơn lẻ ra ngoài (TMP-03).
- Các dãy đồi xa không nhạt dần, không ngả xanh lam (TMP-01).

### TMP-01 · Ridge upon ridge of forest, Great Smoky Mountains National Park

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Great_Smoky_Mountains_National_Park_GRSM1783.jpg) · Xem trước: [1280×853, 370 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/2/21/Great_Smoky_Mountains_National_Park_GRSM1783.jpg/1280px-Great_Smoky_Mountains_National_Park_GRSM1783.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: National Park Service Digital Image Archives
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a National Park Service employee (US federal government work)
- Milestone: R3, R4, P1
- Tiêu chí thị giác:
  - forest covers every ridge continuously; ridges stack with each farther one lighter and bluer
  - sunlit and shaded slopes differ in forest tone (aspect shading), not one flat green
  - ridge profiles are rounded and old, not spiky

### TMP-02 · Beech, ostrich ferns and wood horsetail in Gullmarsskogen nature reserve, Sweden

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Beech_and_ferns_in_Gullmarsskogen.jpg) · Xem trước: [1280×853, 544 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/b/bb/Beech_and_ferns_in_Gullmarsskogen.jpg/1280px-Beech_and_ferns_in_Gullmarsskogen.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: W.carter (own work)
- Giấy phép: **CC0** (Creative Commons Zero, Public Domain Dedication). Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): CC0 1.0 Universal Public Domain Dedication
- Milestone: R4, R3
- Tiêu chí thị giác:
  - the forest floor is covered by understorey (ferns, herbs), not bare ground under trees
  - trunks vary in diameter, lean and spacing; no cloned trees
  - light under the canopy is dappled and green-tinted
- Ghi chú: Commons Quality image.

### TMP-03 · Wooded hills in autumn colours above a meadow, Quebec City

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Autumn_forest_near_Avenue_du_Bourg-Royal,_Quebec_City.jpg) · Xem trước: [1280×634, 333 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/b/b0/Autumn_forest_near_Avenue_du_Bourg-Royal%2C_Quebec_City.jpg/1280px-Autumn_forest_near_Avenue_du_Bourg-Royal%2C_Quebec_City.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Wilfredor (own work)
- Giấy phép: **CC0** (Creative Commons Zero, Public Domain Dedication). Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): CC0 1.0 Universal Public Domain Dedication
- Milestone: R4, R1
- Tiêu chí thị giác:
  - autumn colour mixes red, orange, yellow and evergreen crown by crown (fine-grained), not one biome-wide colour
  - the forest-meadow edge is irregular with shrubs and outliers, not a straight boundary
  - at distance crowns read as a bumpy rounded texture
- Ghi chú: Commons Quality image; wide aspect (preview 1280x634).

### TMP-04 · Nature reserve forest (HDRI) - mixed forest at low sun

- Nguồn: [Poly Haven](https://polyhaven.com/a/nature_reserve_forest) · Xem trước: [630 px ngang, 622 KB](https://cdn.polyhaven.com/asset_img/thumbs/nature_reserve_forest.png?width=630&v=991131ae)
- Tác giả / ghi công: Dimitrios Savva (photography), Jarod Guest (processing) / Poly Haven
- Giấy phép: **CC0** (CC0 1.0 Universal - public domain dedication, no attribution required). Bằng chứng: Asset page (opened 2026-11-20) declares licence https://creativecommons.org/publicdomain/zero/1.0/ ('CC0 1.0 Universal - public domain dedication, no attribution required'); https://polyhaven.com/license states all Poly Haven assets are CC0.
- Milestone: R4, R3
- Tiêu chí thị giác:
  - conifer and broadleaf silhouettes occur in the same stand (mixed forest)
  - low sun rim-lights crowns while the undergrowth stays cool and dark
- Ghi chú: 360-degree equirectangular HDRI panorama.

## 5. Taiga / boreal forest

**Bản đồ Anima hiện đang trượt ở đâu:**

- Không có khảm vá giữa vân sam sẫm và cây lá rụng vàng; cả vùng một màu (TAI-01).
- Mật độ rừng không gắn với nước: thực tế rừng dày dọc sông, thưa hoặc thành đầm lầy ở xa, nhiều ao hồ nhỏ (TAI-02).
- Cây không có dáng chóp nón hẹp, khoảng cách không ngẫu nhiên (TAI-03).
- Không có tuyết đọng trên cành và trên nền dưới tán thưa (TAI-03).

### TAI-01 · Fall in the boreal forest of Nowitna NWR, Alaska (aerial; Yukon River, Kokrine Hills)

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Fall_in_the_boreal_forest_of_Nowitna_National_Wildlife_Refuge_(51124330979).jpg) · Xem trước: [1280×853, 486 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/7/7d/Fall_in_the_boreal_forest_of_Nowitna_National_Wildlife_Refuge_%2851124330979%29.jpg/1280px-Fall_in_the_boreal_forest_of_Nowitna_National_Wildlife_Refuge_%2851124330979%29.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Keith Ramos / USFWS
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a U.S. Fish and Wildlife Service employee (US federal government work); Flickr source reviewed by FlickreviewR: Public Domain Mark
- Milestone: R4, P1, R2
- Tiêu chí thị giác:
  - boreal mosaic: dark spruce patches interleaved with yellow deciduous patches; patch edges irregular
  - a large river with sandbars runs along the base of the hills
  - background hills are lower in contrast and bluer
- Ghi chú: Aerial view.

### TAI-02 · Riparian boreal forest in spring, Yukon Delta NWR, Alaska (aerial)

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Riparian_boreal_forest_(9527251560).jpg) · Xem trước: [1280×920, 437 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/6/6d/Riparian_boreal_forest_%289527251560%29.jpg/1280px-Riparian_boreal_forest_%289527251560%29.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Kristine Sowl / USFWS
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a U.S. Fish and Wildlife Service employee (US federal government work); Flickr source reviewed by FlickreviewR: Public Domain Mark
- Milestone: P1, R4, R2
- Tiêu chí thị giác:
  - tree cover follows the water: denser, taller forest along channels, sparser or wetland away from them
  - flat terrain is dotted with many small ponds and lakes
  - vegetation bands run parallel to the river channel
- Ghi chú: Aerial view.

### TAI-03 · Black spruce forest under snow, White Mountains NRA, Alaska

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Snowy_boreal_forest_(51267438559).jpg) · Xem trước: [1280×759, 283 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/a/a6/Snowy_boreal_forest_%2851267438559%29.jpg/1280px-Snowy_boreal_forest_%2851267438559%29.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Craig McCaa / BLM Alaska
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a Bureau of Land Management employee (US federal government work); Flickr source reviewed by FlickreviewR: Public Domain Mark
- Milestone: R4, R1, R3
- Tiêu chí thị giác:
  - narrow conical spruce crowns, irregularly spaced, with snow visible between trees
  - branches hold snow; ground fully white under an open canopy
  - low winter light with long blue shadows on snow

## 6. Tundra

**Bản đồ Anima hiện đang trượt ở đâu:**

- Ranh giới với taiga hoặc núi là đường cứng; thực tế chuyển màu dần và hàng cây (treeline) lởm chởm (TUN-01, MTN-01).
- Đồi nhọn kiểu low-poly; thực tế đồi tròn, nhẵn (TUN-01).
- Một màu phẳng; thực tế thảm thấp đổi xanh-ô liu-nâu liên tục, đá lộ ở chỗ dốc (TUN-01, TUN-03).
- Thiếu mạng sông bện và ao hồ dày đặc trên đồng bằng lãnh nguyên (AER-08).

### TUN-01 · Green summer tundra and the Mulgrave Hills, Cape Krusenstern NM, Alaska

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Tundra_and_Mulgrave_Hills_(933fedb0-1dd8-b71c-079d-bd390237f5f2).jpg) · Xem trước: [1280×960, 168 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/e/eb/Tundra_and_Mulgrave_Hills_%28933fedb0-1dd8-b71c-079d-bd390237f5f2%29.jpg/1280px-Tundra_and_Mulgrave_Hills_%28933fedb0-1dd8-b71c-079d-bd390237f5f2%29.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: NPS Photo
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a National Park Service employee (US federal government work)
- Milestone: P1, R1, R4
- Tiêu chí thị giác:
  - treeless rounded hills covered by a low continuous vegetation mat
  - colour varies smoothly green to olive/brown across slopes; no hard biome edge
  - hill forms are smooth and rounded, not spiky

### TUN-02 · Tundra looking south toward the Brooks Range, Arctic NWR 1002 Area

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Tundra_looking_south_toward_Brooks_Range,_Alaska.jpg) · Xem trước: [1205×800, 244 KB](https://upload.wikimedia.org/wikipedia/commons/6/67/Tundra_looking_south_toward_Brooks_Range%2C_Alaska.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=original)
- Tác giả / ghi công: U.S. Fish & Wildlife Service
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a U.S. Fish and Wildlife Service employee (US federal government work)
- Milestone: R4, R3, P1
- Tiêu chí thị giác:
  - flat coastal-plain tundra rising to mountains on the horizon
  - far mountains are desaturated and bluish
  - near-ground texture is low shrub/tussock, no trees anywhere
- Ghi chú: Preview is the original file (1205x800).

### TUN-03 · Mountainous tundra landscape

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Mountainous_Tundra_Landscape_(45380376195).jpg) · Xem trước: [1280×855, 295 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/b/b5/Mountainous_Tundra_Landscape_%2845380376195%29.jpg/1280px-Mountainous_Tundra_Landscape_%2845380376195%29.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Janet Prevey / Pacific Northwest Research Station, USDA Forest Service
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of the USDA Forest Service (US federal government work); Flickr source reviewed by FlickreviewR: Public Domain Mark
- Milestone: R1, P1
- Tiêu chí thị giác:
  - vegetation cover thins with elevation into rock, scree and bare ground
  - tundra mat follows terrain smoothly; rock shows through on steeper ground

## 7. Swamp, marsh, mangrove

**Bản đồ Anima hiện đang trượt ở đâu:**

- Nước là mặt phẳng một màu; thực tế nước sẫm màu trà hoặc đục nâu xanh, có phản chiếu, cây đứng ngay trong nước (WET-01, WET-03).
- Ranh giới đất-nước cứng và trơn; thực tế lởm chởm theo lạch triều (WET-03, WET-04).
- Không có cấu trúc đảo cây hình vòm giữa đồng cỏ lác (WET-02).
- Rừng ngập mặn không bám mép nước và mạng lạch triều phân nhánh (WET-04).

### WET-01 · Bald cypresses in Okefenokee Swamp

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Bald_Cypress_swamp.jpg) · Xem trước: [1280×846, 381 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/e/ee/Bald_Cypress_swamp.jpg/1280px-Bald_Cypress_swamp.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Ryan Hagerty / USFWS
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a U.S. Fish and Wildlife Service employee (US federal government work)
- Milestone: R2, R4
- Tiêu chí thị giác:
  - trees stand directly in still, dark water; calm surface reflects trunks
  - trunk bases flare at the waterline
  - water is dark/tea-coloured, not a bright blue plane

### WET-02 · Cypress dome in sawgrass marsh, Everglades National Park

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Cypress_dome_in_Everglades_(53409344060).jpg) · Xem trước: [1280×853, 457 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/c/cb/Cypress_dome_in_Everglades_%2853409344060%29.jpg/1280px-Cypress_dome_in_Everglades_%2853409344060%29.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: NPS (evergladesnps)
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a National Park Service employee (US federal government work); Flickr source reviewed by FlickreviewR: Public Domain Mark
- Milestone: P1, R4, R2
- Tiêu chí thị giác:
  - dome-shaped tree island: tallest trees in the centre, lower toward the edge, rising from flat marsh
  - open marsh shows tonal variation (sawgrass vs. wetter sloughs), not one flat colour
  - horizon nearly flat; sky dominates the frame

### WET-03 · Hara mangrove forests near Jask, Hormozgan province, Iran

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Hara_mangrove_forrests_in_Hormozgan_province,_southern_Iran.jpg) · Xem trước: [1280×854, 479 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/2/2b/Hara_mangrove_forrests_in_Hormozgan_province%2C_southern_Iran.jpg/1280px-Hara_mangrove_forrests_in_Hormozgan_province%2C_southern_Iran.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Okruz (own work)
- Giấy phép: **CC0** (Creative Commons Zero, Public Domain Dedication). Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): CC0 1.0 Universal Public Domain Dedication
- Milestone: R4, R2
- Tiêu chí thị giác:
  - mangrove forms a dense low canopy right at the waterline of tidal channels
  - the water-vegetation boundary is ragged and follows tidal creeks
  - coastal water is turbid green-brown, not deep blue

### WET-04 · Brahmani delta: mangroves and beach ridges (ISS astronaut photo)

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Aerial_satellite_imagery_of_Brahmani_delta,_mangrove_and_beach_ridges.JPG) · Xem trước: [1280×850, 453 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/f/fd/Aerial_satellite_imagery_of_Brahmani_delta%2C_mangrove_and_beach_ridges.JPG/1280px-Aerial_satellite_imagery_of_Brahmani_delta%2C_mangrove_and_beach_ridges.JPG?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: NASA (ISS030 crew photo)
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: solely created by NASA (NASA material not protected by copyright unless noted)
- Milestone: P1, R2
- Tiêu chí thị giác:
  - mangrove belt follows a branching tidal-channel network near the coast
  - beach ridges run parallel to the shoreline behind the beach
  - sediment colours coastal water brown near river mouths
- Ghi chú: Orbital oblique view.

## 8. High mountains, snow, glaciers (treeline, snowline)

**Bản đồ Anima hiện đang trượt ở đâu:**

- Núi là chóp nhọn; thực tế đỉnh lởm chởm đứng trên vạt đá vụn (talus), sông băng nằm trong lòng chảo (cirque) và trục thung lũng (MTN-01, MTN-03, MTN-04).
- Tuyết là một mũ màu cố định; thực tế có snowline theo độ cao, tuyết không bám mặt quá dốc, đọng lại trong khe và sườn khuất (MTN-02, MTN-06).
- Treeline là một đường đồng mức cứng; thực tế ngón tay cây vươn lên theo sống và khe (MTN-01, MTN-05).
- Không có vật liệu sông băng và băng tích (MTN-03).
- Dãy núi xa không có phối cảnh khí quyển (MTN-06, ATM-01).

### MTN-01 · A snowy Tatoosh Range rises above the treeline, Mount Rainier NP

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:A_snowy_Tatoosh_Range_rises_above_the_treeline._(bd00c08e-cdcf-4e34-a9b3-90796d8af6ce).JPEG) · Xem trước: [1280×853, 323 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/8/80/A_snowy_Tatoosh_Range_rises_above_the_treeline._%28bd00c08e-cdcf-4e34-a9b3-90796d8af6ce%29.JPEG/1280px-A_snowy_Tatoosh_Range_rises_above_the_treeline._%28bd00c08e-cdcf-4e34-a9b3-90796d8af6ce%29.JPEG?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: NPS Photo
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a National Park Service employee (US federal government work)
- Milestone: P1, R1, R4
- Tiêu chí thị giác:
  - TREELINE: ragged and fingered (trees push up ridges and gullies), not a contour line
  - above the treeline only rock and snow; snow lingers in gullies and shaded faces
  - peaks are jagged but sit on talus/scree aprons
- Ghi chú: Treeline reference.

### MTN-02 · Termination dust (first autumn snow) in the Chugach Mountains, Alaska

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Termination_Dust.jpg) · Xem trước: [1280×960, 288 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/6/60/Termination_Dust.jpg/1280px-Termination_Dust.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Williamkholeif (own work)
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: released into the public domain worldwide by the copyright holder
- Milestone: P1, R1
- Tiêu chí thị giác:
  - SNOWLINE: fresh snow ends at a near-constant elevation across many slopes (a horizontal band)
  - below the snowline autumn vegetation colours, above it white
  - the snowline edge is slightly patchy over tens of metres, not razor-sharp
- Ghi chú: Snowline reference. Released to the public domain by the author (Commons PD template).

### MTN-03 · The Alaska Range from Unit 13, Denali backcountry: rock-covered glacier between snow-capped mountains

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:The_Alaska_Range_seen_from_Unit_13_in_Denali%27s_backcountry_on_June_13,_2019._(6ff804de-cbba-4e96-bbe4-738a0070cdc8).JPG) · Xem trước: [1280×831, 317 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/c/c4/The_Alaska_Range_seen_from_Unit_13_in_Denali%27s_backcountry_on_June_13%2C_2019._%286ff804de-cbba-4e96-bbe4-738a0070cdc8%29.JPG/1280px-The_Alaska_Range_seen_from_Unit_13_in_Denali%27s_backcountry_on_June_13%2C_2019._%286ff804de-cbba-4e96-bbe4-738a0070cdc8%29.JPG?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Emily Mesner / NPS
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a National Park Service employee (US federal government work)
- Milestone: P1, R1
- Tiêu chí thị giác:
  - valley glacier surface is covered in dark rock debris (moraine bands), not clean white
  - snow sticks to gentler slopes; the steepest rock faces stay bare
  - the glacier occupies the valley axis, the lowest line of the valley
- Ghi chú: Glacier reference.

### MTN-04 · Ladder Creek and Neve Glaciers, Snowfield Peak, North Cascades NP

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Neve_Glacier_North_Cascades_USGS.jpg) · Xem trước: [1280×1116, 392 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/f/f0/Neve_Glacier_North_Cascades_USGS.jpg/1280px-Neve_Glacier_North_Cascades_USGS.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: M. F. Meier / U.S. Geological Survey
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: material from the U.S. Geological Survey
- Milestone: P1, R1, R4
- Tiêu chí thị giác:
  - glacierized peaks rise abruptly above heavily forested valleys (<1000 m elevation)
  - forest is continuous in the valleys and thins upward to alpine rock and ice
  - glaciers sit in cirques/basins below ridges and show crevasse texture
- Ghi chú: Treeline + glacier in one frame; historical USGS Photographic Library image.

### MTN-05 · Mount Taranaki's Ring of Forest (Landsat 8 OLI, 10 June 2023)

- Nguồn: [NASA Earth Observatory](https://science.nasa.gov/earth/earth-observatory/mount-taranakis-ring-of-forest-153342) · Xem trước: [kích thước chưa rõ, 231 KB](https://assets.science.nasa.gov/content/dam/science/esd/eo/images/imagerecords/153000/153342/mttaranaki_oli_20230610.jpg)
- Tác giả / ghi công: NASA Earth Observatory image by Wanmei Liang, using Landsat data from the U.S. Geological Survey
- Giấy phép: **Public domain (NASA content, not subject to US copyright)** (NASA Images and Media Usage Guidelines). Bằng chứng: EO page credit (opened 2026-11-20): 'NASA Earth Observatory image by Wanmei Liang, using Landsat data from the U.S. Geological Survey'. NASA guidelines page https://www.nasa.gov/nasa-brand-center/images-and-media/ : NASA images generally are not subject to copyright in the US; third-party material is marked. Exception on this page: the ground photo by Kathrin & Stefan Marks is CC BY-NC-ND 2.0 and is excluded. Commons mirror tagged Public domain (NASA).
- Milestone: P1, R1, R4
- Tiêu chí thị giác:
  - from above: snow cap, bare rock/scree, alpine scrub and dark forest form concentric zones that follow elevation
  - radial gullies are carved into the cone
  - natural zone boundaries are irregular; the only perfectly sharp circular edge is the human park boundary (forest vs pasture)
- Ghi chú: The EO page also shows a ground photo by Kathrin & Stefan Marks under CC BY-NC-ND 2.0; that photo is NOT part of this entry. Commons mirror listed under alt_source.
- Bản sao trên Commons: [https://commons.wikimedia.org/wiki/File:Mount_Taranaki%E2%80%99s_Ring_of_Forest_(153342).jpg](https://commons.wikimedia.org/wiki/File:Mount_Taranaki%E2%80%99s_Ring_of_Forest_(153342).jpg) · [xem trước 1280×853, 422 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/f/fa/Mount_Taranaki%E2%80%99s_Ring_of_Forest_%28153342%29.jpg/1280px-Mount_Taranaki%E2%80%99s_Ring_of_Forest_%28153342%29.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail) · Public domain

### MTN-06 · Mont Turia and Mont Pourri seen from the Aiguille Rouge, Les Arcs, French Alps

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Mont_Turia_and_Mont_Pourri_seen_from_the_Aiguille_Rouge,_Les_Arcs,_2017.jpg) · Xem trước: [1280×851, 268 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/0/0a/Mont_Turia_and_Mont_Pourri_seen_from_the_Aiguille_Rouge%2C_Les_Arcs%2C_2017.jpg/1280px-Mont_Turia_and_Mont_Pourri_seen_from_the_Aiguille_Rouge%2C_Les_Arcs%2C_2017.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: DimiTalen (own work)
- Giấy phép: **CC0** (Creative Commons Zero, Public Domain Dedication). Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): CC0 1.0 Universal Public Domain Dedication
- Milestone: P1, R1, R3
- Tiêu chí thị giác:
  - glacier tongues and snowfields sit in high basins with bare grey rock ridges between them
  - snow is absent on the steepest faces and present on gentler slopes and ledges
  - distant summits are hazier and bluer than near ones
- Ghi chú: Commons Quality image.

## 9. Coast and beach (rocky coast, surf, foam)

**Bản đồ Anima hiện đang trượt ở đâu:**

- Mặt nước phẳng chạm đất ở một mép cứng: không có dải bọt, không có các hàng sóng vỡ (CST-01, CST-03).
- Không có dải cát/đá ướt sẫm và bóng hơn ở mép nước (CST-01, CST-04).
- Màu nước đồng nhất; thực tế nông-sâu đổi màu, đục ở cửa sông (CST-04, AER-04).
- Thiếu địa hình bờ: vách, cột đá biển, núi đổ thẳng xuống sóng (CST-02, CST-03).

### CST-01 · Big surf morning, James Island, Rialto Beach, Olympic NP

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Big_surf_morning_james_island_rialto_beach_summer_d_archuleta_2015_(23796781053).jpg) · Xem trước: [1280×853, 200 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/6/69/Big_surf_morning_james_island_rialto_beach_summer_d_archuleta_2015_%2823796781053%29.jpg/1280px-Big_surf_morning_james_island_rialto_beach_summer_d_archuleta_2015_%2823796781053%29.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: D. Archuleta / Olympic National Park (NPS)
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a National Park Service employee (US federal government work); Flickr source reviewed by FlickreviewR: Public Domain Mark
- Milestone: R2, R3
- Tiêu chí thị giác:
  - several parallel breaker lines, each followed by a white foam band; foam thins toward the beach
  - spray/mist lowers contrast of the offshore island behind the surf
  - wet sand near the swash line is darker and reflective
- Ghi chú: Surf/foam reference.

### CST-02 · Sea stacks and crashing waves at Second Beach, Olympic Coast NMS

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:OCNMS_-_Surfers_At_Second_Beach_(30550423453).jpg) · Xem trước: [1280×721, 291 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/0/04/OCNMS_-_Surfers_At_Second_Beach_%2830550423453%29.jpg/1280px-OCNMS_-_Surfers_At_Second_Beach_%2830550423453%29.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Matt McIntosh / NOAA
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: material from NOAA made as part of an employee's official duties; Flickr source reviewed by FlickreviewR: Public Domain Mark
- Milestone: R2, R3
- Tiêu chí thị giác:
  - white foam forms where waves break; foam streaks trail behind breakers
  - sea stacks rise straight out of the surf with wave-cut bases
  - sunset sky reflects on wet sand and water

### CST-03 · Black Sand Beach, King Range NCA, California (Lost Coast)

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Black_Sand_Beach_(54103811794).jpg) · Xem trước: [1280×782, 269 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/d/db/Black_Sand_Beach_%2854103811794%29.jpg/1280px-Black_Sand_Beach_%2854103811794%29.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: BLM California
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a Bureau of Land Management employee (US federal government work); Flickr source reviewed by FlickreviewR: Public Domain Mark
- Milestone: P1, R2, R1
- Tiêu chí thị giác:
  - mountains rise steeply straight out of the surf; no wide flat coastal plain
  - surf zone shows multiple breaker lines with foam between them
  - dark sand beach is a narrow strip between surf and slope

### CST-04 · Simon's Town rocks (HDRI) - rocky coast in morning sun

- Nguồn: [Poly Haven](https://polyhaven.com/a/simons_town_rocks) · Xem trước: [630 px ngang, 399 KB](https://cdn.polyhaven.com/asset_img/thumbs/simons_town_rocks.png?width=630&v=4349bf17)
- Tác giả / ghi công: Greg Zaal (HDRI), Rico Cilliers (backplates) / Poly Haven
- Giấy phép: **CC0** (CC0 1.0 Universal - public domain dedication, no attribution required). Bằng chứng: Asset page (opened 2026-11-20) declares licence https://creativecommons.org/publicdomain/zero/1.0/ ('CC0 1.0 Universal - public domain dedication, no attribution required'); https://polyhaven.com/license states all Poly Haven assets are CC0.
- Milestone: R2, R1, R3
- Tiêu chí thị giác:
  - rocks near the waterline are darker and glossier (wet) than dry rocks above
  - shallow water near shore is clearer/greener than deep water farther out
- Ghi chú: 360-degree equirectangular HDRI panorama.

## 10. River valleys and waterfalls

**Bản đồ Anima hiện đang trượt ở đâu:**

- Sông là dải sơn có độ rộng không đổi; thực tế rộng dần về hạ lưu, uốn khúc với bãi bồi bờ lồi và hồ móng ngựa (RIV-01, AER-07).
- Sông không khoét thung lũng; đáy thung lũng không phẳng hơn và xanh hơn sống đồi (RIV-03, RIV-04).
- Không có thác: không có điểm gãy lòng sông, không có nước trắng và bụi nước (RIV-02, RIV-03).

### RIV-01 · Meandering river, Innoko National Wildlife Refuge, Alaska (aerial)

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Meandering_River_2_Innoko_NWR.jpg) · Xem trước: [1280×861, 180 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/7/72/Meandering_River_2_Innoko_NWR.jpg/1280px-Meandering_River_2_Innoko_NWR.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: U.S. Fish and Wildlife Service
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a U.S. Fish and Wildlife Service employee (US federal government work)
- Milestone: P1, R2
- Tiêu chí thị giác:
  - MEANDER: sinuous loops with point bars on inner bends and cut banks on outer bends
  - abandoned loops / oxbow lakes and scroll bars lie beside the active channel
  - channel width stays roughly constant along the reach; floodplain flat and forested
- Ghi chú: Meandering-river reference.

### RIV-02 · Montmorency Falls, Quebec

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Parc_de_la_Chute-Montmorency_001.jpg) · Xem trước: [1280×733, 332 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/f/ff/Parc_de_la_Chute-Montmorency_001.jpg/1280px-Parc_de_la_Chute-Montmorency_001.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Wilfredor (own work)
- Giấy phép: **CC0** (Creative Commons Zero, Public Domain Dedication). Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): CC0 1.0 Universal Public Domain Dedication
- Milestone: P1, R2
- Tiêu chí thị giác:
  - WATERFALL: water turns white and opaque as it falls, with mist at the base
  - the river drops over a cliff into a plunge pool and widens downstream
  - the cliff face around the fall is bare, wet, dark rock
- Ghi chú: Commons Quality image (original 15189 px wide).

### RIV-03 · Ulaan Tsutgalan waterfall, Orkhon Valley, Mongolia

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Ulaan_Tsutgalan_Waterfall.jpg) · Xem trước: [1280×762, 521 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/3/38/Ulaan_Tsutgalan_Waterfall.jpg/1280px-Ulaan_Tsutgalan_Waterfall.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Bernard Gagnon (own work)
- Giấy phép: **CC0** (Creative Commons Zero, Public Domain Dedication). Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): CC0 1.0 Universal Public Domain Dedication
- Milestone: P1, R2, R1
- Tiêu chí thị giác:
  - the river is incised into a gorge while the surrounding steppe plateau stays flat
  - wet dark rock and white spray around the fall

### RIV-04 · View of the Muhr Valley towards Pichlern, Lungau, Austria

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:View_of_the_Muhr_Valley_towards_Pichlern_from_the_B99_at_Staig,_St._Margarethen_im_Lungau,_2023.jpg) · Xem trước: [1280×851, 183 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/4/4d/View_of_the_Muhr_Valley_towards_Pichlern_from_the_B99_at_Staig%2C_St._Margarethen_im_Lungau%2C_2023.jpg/1280px-View_of_the_Muhr_Valley_towards_Pichlern_from_the_B99_at_Staig%2C_St._Margarethen_im_Lungau%2C_2023.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: DimiTalen (own work)
- Giấy phép: **CC0** (Creative Commons Zero, Public Domain Dedication). Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): CC0 1.0 Universal Public Domain Dedication
- Milestone: P1, R1, R4, R3
- Tiêu chí thị giác:
  - flat green valley floor with river and fields, forested lower slopes, steeper bare upper slopes
  - valley floor greener than ridges
  - far end of the valley hazier than the near slopes

## 11. Lakes in a landscape

**Bản đồ Anima hiện đang trượt ở đâu:**

- Hồ là mặt phẳng xanh dương, không chắc nằm ở điểm thấp nhất lưu vực; thực tế bờ hồ đi theo đường đồng mức (LAK-02, LAK-03).
- Không có chuyển màu theo độ sâu, không có vệt gió và phản chiếu (LAK-01, LAK-02).
- Chuyển tiếp bờ cứng; thực tế vách dốc với vạt đá vụn, hoặc bờ cỏ đá trên treeline (LAK-01, LAK-02).

### LAK-01 · Crater Lake in summer: Phantom Ship with Mount Scott, from Garfield Peak

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Crater_Lake_in_Summer_2,_Crater_Lake_National_Park.jpg) · Xem trước: [1280×960, 436 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/a/a9/Crater_Lake_in_Summer_2%2C_Crater_Lake_National_Park.jpg/1280px-Crater_Lake_in_Summer_2%2C_Crater_Lake_National_Park.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: National Park Service
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a National Park Service employee (US federal government work)
- Milestone: R2, P1
- Tiêu chí thị giác:
  - deep water is a saturated dark blue that lightens only near the shore
  - caldera walls drop steeply into the water with talus at their foot
  - the lake surface is not a mirror-flat single colour: wind ripples change tone in patches

### LAK-02 · Lac du Lou from below Col de la Chambre, Les Menuires, French Alps

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Lac_du_Lou_from_below_Col_de_la_Chambre,_Les_Menuires.jpg) · Xem trước: [1280×851, 306 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/4/44/Lac_du_Lou_from_below_Col_de_la_Chambre%2C_Les_Menuires.jpg/1280px-Lac_du_Lou_from_below_Col_de_la_Chambre%2C_Les_Menuires.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: DimiTalen (own work)
- Giấy phép: **CC0** (Creative Commons Zero, Public Domain Dedication). Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): CC0 1.0 Universal Public Domain Dedication
- Milestone: P1, R2
- Tiêu chí thị giác:
  - alpine lake sits at the lowest point of its basin; the shoreline follows a contour
  - surroundings are grass and rock with no trees (above treeline)
  - calm water reflects the surrounding slopes
- Ghi chú: Commons Quality image.

### LAK-03 · Waterton Lakes National Park

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Waterton_Lakes_National_Park_(48490188676).jpg) · Xem trước: [1280×853, 293 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/b/b5/Waterton_Lakes_National_Park_%2848490188676%29.jpg/1280px-Waterton_Lakes_National_Park_%2848490188676%29.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Glacier NPS (Flickr)
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a National Park Service employee (US federal government work); Flickr source reviewed by FlickreviewR: Public Domain Mark
- Milestone: P1, R2, R4
- Tiêu chí thị giác:
  - long narrow glacial lake filling a trough between steep mountains
  - forested lower slopes, bare rock higher up
  - water tone varies with depth and wind (ripples vs. calm patches)

## 12. Aerial/satellite drainage (G-SHAPE)

**Bản đồ Anima hiện đang trượt ở đâu:**

- Sông không tạo cây phân nhánh dạng cành (dendritic): nhánh phụ phải nhập ở góc nhọn và bậc dòng tăng về hạ lưu (AER-01, AER-02).
- Không có quạt bồi tích ở chân núi, không có châu thổ ở bờ biển (AER-03, AER-04, AER-05).
- Xói mòn không tập trung theo mạng thoát nước: sống núi là nhiễu nhọn thay vì sống dao giữa các khe (AER-06).
- Bề mặt cao nguyên giữa các khe không được giữ lại (AER-01, AER-06).
- Không có sông bện và hồ móng ngựa trên đồng bằng (AER-07, AER-08).

### AER-01 · Hadramawt Plateau, Yemen: dendritic Wadi Hadramawt (Gemini 4 photo)

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:S65-34658_The_Hadramawt_Plateau,_Yemen_on_Earth.jpg) · Xem trước: [1280×1245, 463 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/a/af/S65-34658_The_Hadramawt_Plateau%2C_Yemen_on_Earth.jpg/1280px-S65-34658_The_Hadramawt_Plateau%2C_Yemen_on_Earth.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: NASA (Gemini 4, S65-34658)
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: solely created by NASA (NASA material not protected by copyright unless noted)
- Milestone: P1
- Tiêu chí thị giác:
  - DENDRITIC: tributaries join at acute angles and channel order grows downstream
  - plateau surfaces between wadis stay flat and intact; erosion is concentrated in channels
  - drainage density is roughly uniform over uniform rock
- Ghi chú: G-SHAPE reference; oblique 1965 film photo.

### AER-02 · Drainage patterns in the Empty Quarter, Saudi Arabia (STS-55)

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:STS055-16-009_-_STS-055_-_Detail_Views_of_Drainage_Patterns_in_the_Empty_Quarter,_Saudi_Arabia_-_DPLA_-_e66289bf78417e61a5a6cc7b98c40930.jpg) · Xem trước: [1280×805, 140 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/d/d7/STS055-16-009_-_STS-055_-_Detail_Views_of_Drainage_Patterns_in_the_Empty_Quarter%2C_Saudi_Arabia_-_DPLA_-_e66289bf78417e61a5a6cc7b98c40930.jpg/1280px-STS055-16-009_-_STS-055_-_Detail_Views_of_Drainage_Patterns_in_the_Empty_Quarter%2C_Saudi_Arabia_-_DPLA_-_e66289bf78417e61a5a6cc7b98c40930.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: NASA Johnson Space Center (via U.S. National Archives)
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: solely created by NASA (NASA material not protected by copyright unless noted)
- Milestone: P1
- Tiêu chí thị giác:
  - every channel joins a larger one (tree topology, no loops, no parallel artefacts)
  - channel floors and interfluves differ in tone along the whole network
- Ghi chú: G-SHAPE reference; exact location unknown per NASA caption.

### AER-03 · Alluvial fan between the Kunlun and Altun ranges, Taklimakan Desert (ASTER)

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Alluvial_fan,_Taklimakan_Desert,_XinJiang_Province,_China,_NASA,_ASTER.jpg) · Xem trước: [1280×1386, 729 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/7/7d/Alluvial_fan%2C_Taklimakan_Desert%2C_XinJiang_Province%2C_China%2C_NASA%2C_ASTER.jpg/1280px-Alluvial_fan%2C_Taklimakan_Desert%2C_XinJiang_Province%2C_China%2C_NASA%2C_ASTER.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: NASA/GSFC/METI/ERSDAC/JAROS and U.S./Japan ASTER Science Team
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: solely created by NASA (NASA material not protected by copyright unless noted)
- Milestone: P1
- Tiêu chí thị giác:
  - ALLUVIAL FAN: radiates from one apex where the river leaves the mountains
  - distributary channels braid and spread downslope; only one sector is active (wet)
  - mountains behind the apex are deeply dissected, fan surface smooth
- Ghi chú: Commons Featured picture. ASTER colour composite (water appears blue).

### AER-04 · Building up the Yellow River delta (Landsat)

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Building_Up_the_Yellow_River_Delta.jpeg) · Xem trước: [1280×1422, 801 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/2/28/Building_Up_the_Yellow_River_Delta.jpeg/1280px-Building_Up_the_Yellow_River_Delta.jpeg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: NASA Earth Observatory images by Lauren Dauphin, using Landsat data from the USGS
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: solely created by NASA (NASA material not protected by copyright unless noted)
- Milestone: P1, R2
- Tiêu chí thị giác:
  - DELTA: distributary channels fork downstream (opposite of tributary joins)
  - the delta lobe protrudes into the sea with a sediment plume at the active mouth
  - coastline is irregular with tidal flats
- Ghi chú: Commons Featured picture.

### AER-05 · The meandering estuaries of Guinea-Bissau (Landsat 8 OLI)

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Guineabissau_oli_2018137_lrg.jpg) · Xem trước: [1280×1049, 549 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/1/1a/Guineabissau_oli_2018137_lrg.jpg/1280px-Guineabissau_oli_2018137_lrg.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: NASA Earth Observatory images by Joshua Stevens, using Landsat data from the USGS
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: solely created by NASA (NASA material not protected by copyright unless noted)
- Milestone: P1, R2
- Tiêu chí thị giác:
  - tidal channels branch like roots; channel width shrinks quickly inland
  - sediment-laden water is lighter brown near the coast, darker offshore
- Ghi chú: Commons Featured picture.

### AER-06 · The dizzying depths of Cotahuasi Canyon, Peru (Landsat 8, oblique)

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:The_Dizzying_Depths_of_Cotahuasi_Canyon_(153210_-_cotahuasi_oli_2016155_oblique_lrg).jpg) · Xem trước: [1280×960, 493 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/e/ea/The_Dizzying_Depths_of_Cotahuasi_Canyon_%28153210_-_cotahuasi_oli_2016155_oblique_lrg%29.jpg/1280px-The_Dizzying_Depths_of_Cotahuasi_Canyon_%28153210_-_cotahuasi_oli_2016155_oblique_lrg%29.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: NASA Earth Observatory images by Joshua Stevens, using Landsat data from the USGS
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: solely created by NASA (NASA material not protected by copyright unless noted); Public domain: material from the U.S. Geological Survey
- Milestone: P1, R1
- Tiêu chí thị giác:
  - ERODED RIDGES: tributary canyons leave knife-edge ridges between them
  - the plateau surface survives above canyon rims; erosion concentrates along drainage
  - snow only on the highest volcanic peaks above the plateau
- Ghi chú: Oblique view made from Landsat data (NASA Earth Observatory).

### AER-07 · Meandering course of the Red River, Lafayette and Miller counties, Arkansas (NAIP orthophoto)

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:RedRiverMeandersArkansas1.jpg) · Xem trước: [1280×1500, 800 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/5/5e/RedRiverMeandersArkansas1.jpg/1280px-RedRiverMeandersArkansas1.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: USDA Farm Service Agency (NAIP 2010)
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a US federal employee (17 U.S.C. 105)
- Milestone: P1, R2
- Tiêu chí thị giác:
  - point bars, abandoned meander loops, oxbow lakes and ridge-and-swale scroll topography
  - scroll ridges record former channel positions in concentric arcs
  - the active channel is a single thread with wider sections at bends
- Ghi chú: Orthophoto (top-down).

### AER-08 · Arctic wetland: network of rivers intertwined on the tundra, Arctic NWR (aerial)

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Arctic_Wetland_(48753081712).jpg) · Xem trước: [1280×853, 323 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/a/a1/Arctic_Wetland_%2848753081712%29.jpg/1280px-Arctic_Wetland_%2848753081712%29.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Lisa Hupp / USFWS
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a U.S. Fish and Wildlife Service employee (US federal government work); Flickr source reviewed by FlickreviewR: Public Domain Mark
- Milestone: P1, R2
- Tiêu chí thị giác:
  - channels split and rejoin across flat tundra (braided/anastomosing)
  - many ponds; the land-water edge is highly irregular (fractal)

## 13. Atmosphere / aerial perspective / golden hour (optional)

**Bản đồ Anima hiện đang trượt ở đâu:**

- Không có mù khí quyển: núi xa giữ nguyên độ tương phản và độ bão hòa như núi gần (ATM-01, TMP-01).
- Không có ánh sáng giờ vàng: thiếu tách ấm/lạnh giữa mặt sáng và bóng, thiếu bóng đổ dài (ATM-03, ATM-04).
- Không có sương mù đọng trong thung lũng (ATM-02).

### ATM-01 · Glacier Peak and Disappointment Peak from the mouth of the White River (1963 NPS North Cascades survey)

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Glacier_Peak_and_Disappointment_Peak_from_White_River_(ae5685a8-2e0e-4775-977e-07f43d5ede43).tif) · Xem trước: [1280×831, 153 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/6/67/Glacier_Peak_and_Disappointment_Peak_from_White_River_%28ae5685a8-2e0e-4775-977e-07f43d5ede43%29.tif/lossy-page1-1280px-Glacier_Peak_and_Disappointment_Peak_from_White_River_%28ae5685a8-2e0e-4775-977e-07f43d5ede43%29.tif.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: NPS Photo (North Cascades NPS Complex Museum Collection)
- Giấy phép: **Public domain**. Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): Public domain: work of a National Park Service employee (US federal government work)
- Milestone: R3
- Tiêu chí thị giác:
  - AERIAL PERSPECTIVE: at least three ridge layers, each farther one lighter and lower in contrast
  - the far snow peak stays bright but bluish; cloud wraps around the summit
- Ghi chú: NPS North Cascades museum collection (caption cites the 1963 NPS North Cascades survey series); TIFF original, JPEG preview rendition.

### ATM-02 · Mystic layers of Agumbe: mist over forested Western Ghats ridges

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Mystic_Layers_of_Agumbe.jpg) · Xem trước: [1280×964, 436 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/0/09/Mystic_Layers_of_Agumbe.jpg/1280px-Mystic_Layers_of_Agumbe.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Pradyumnakp (own work)
- Giấy phép: **CC0** (Creative Commons Zero, Public Domain Dedication). Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): CC0 1.0 Universal Public Domain Dedication
- Milestone: R3, R4
- Tiêu chí thị giác:
  - mist lies in the valleys, lower than the ridgelines
  - ridge silhouettes fade toward the sky colour with distance
  - dense tropical canopy in the foreground keeps full saturation
- Ghi chú: Also a rainforest-from-above reference.

### ATM-03 · Chepino Valley at early sunset with the Rila Mountains in the distance

- Nguồn: [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Chepino_Valley_Landscape.jpg) · Xem trước: [1280×848, 153 KB](https://thumb.wikimedia.org/wikipedia/commons/thumb/2/27/Chepino_Valley_Landscape.jpg/1280px-Chepino_Valley_Landscape.jpg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo&utm_content=thumbnail)
- Tác giả / ghi công: Nannoyani (own work)
- Giấy phép: **CC0** (Creative Commons Zero, Public Domain Dedication). Bằng chứng: Licence box on the Commons file page (opened 2026-11-20): CC0 1.0 Universal Public Domain Dedication
- Milestone: R3
- Tiêu chí thị giác:
  - GOLDEN HOUR: warm low-angle light on near slopes; distant range bluish and desaturated
  - long shadows from trees and hills fall across the valley floor
- Ghi chú: Commons Quality image.

### ATM-04 · Kiara 8 sunset (HDRI) - golden sunset over rocky ridges and a valley

- Nguồn: [Poly Haven](https://polyhaven.com/a/kiara_8_sunset) · Xem trước: [630 px ngang, 379 KB](https://cdn.polyhaven.com/asset_img/thumbs/kiara_8_sunset.png?width=630&v=608f2ee1)
- Tác giả / ghi công: Greg Zaal / Poly Haven
- Giấy phép: **CC0** (CC0 1.0 Universal - public domain dedication, no attribution required). Bằng chứng: Asset page (opened 2026-11-20) declares licence https://creativecommons.org/publicdomain/zero/1.0/ ('CC0 1.0 Universal - public domain dedication, no attribution required'); https://polyhaven.com/license states all Poly Haven assets are CC0.
- Milestone: R3
- Tiêu chí thị giác:
  - warm/cool split: sunlit faces orange, shadows blue
  - sky gradient from orange at the horizon to deep blue overhead
  - long ridge shadows across the valley
- Ghi chú: 360-degree equirectangular HDRI panorama.

## Chỗ thiếu ảnh CC0/PD đủ tốt

- **Savanna (nhóm 2):** cả 3 ảnh đều là HDRI toàn cảnh của Poly Haven (Nam Phi). Trên Commons không tìm được ảnh phong cảnh savanna châu Phi nào vừa CC0/PD rõ ràng vừa đủ chất lượng: các kết quả CC0 là ảnh thú hoang, ảnh có xe tải, ảnh nguồn Unsplash (bị loại), hoặc mô tả không đủ tin cậy. Thiếu hẳn ảnh savanna nhìn từ trên cao.
- **Rừng mưa nhiệt đới, góc ngang tán (nhóm 3):** chỉ có một ảnh thật sự ở độ cao tán (RNF-03, file gốc nhỏ 1512×1134). Hai ảnh El Yunque của USFS (PD) đã xem mô tả đều có người tắm sông nên không chọn.
- **Bọt sóng nhìn từ trên cao (nhóm 9):** hai ảnh chụp từ máy bay của NOAA ShoreZone rất hợp nhưng trang Commons ghi CC BY 2.0 nên bị loại; hiện chỉ có góc nhìn ngang mặt đất.
- **Rừng ngập mặn nhìn từ trên cao:** chỉ có ảnh chụp từ ISS (WET-04), chưa có ảnh máy bay độ phân giải cao.

## Đã loại sau khi kiểm trang nguồn

- [Espuma_de_playa_en_Bahia_de_Plata.jpg](https://commons.wikimedia.org/wiki/File:Espuma_de_playa_en_Bahia_de_Plata.jpg): Licence box says CC0 but the Permission field still reads 'Multi-license with GFDL and CC-BY-SA-2.5'; mixed, so excluded.
- [Line5070_-_Flickr_-_NOAA_Photo_Library.jpg](https://commons.wikimedia.org/wiki/File:Line5070_-_Flickr_-_NOAA_Photo_Library.jpg): API says public domain but the page licence box is CC BY 2.0 (Flickr); mixed, so excluded.
- [Line5093_-_Flickr_-_NOAA_Photo_Library.jpg](https://commons.wikimedia.org/wiki/File:Line5093_-_Flickr_-_NOAA_Photo_Library.jpg): Same as Line5070: CC BY 2.0 on the page; excluded.
- [Lena_Delta_Shakes_off_Winter_(51283979975).jpg](https://commons.wikimedia.org/wiki/File:Lena_Delta_Shakes_off_Winter_(51283979975).jpg): Page licence box is CC BY 2.0 (NASA Goddard Flickr upload, checked by FlickreviewR); excluded. The PD 'Lena River Delta - Landsat 2000' is false colour and was not used.
- Any Commons file whose name ends in '(Unsplash).jpg': Unsplash-origin; excluded by rule even when tagged CC0.
