---
phase: requirements
feature: map-v2
title: Requirements — Map v2: thế giới chân thực nhất có thể (dữ liệu World v2 và renderer native)
description: Kế hoạch dài hạn đưa bản đồ lên mức chân thực cao nhất có thể. Thế giới được sinh lại bằng xói mòn, thuỷ văn và khí hậu thật, thành artifact ghim hash. Tiếp theo là một bản thử đo đạc để chọn renderer native, rồi dựng renderer qua các mốc có cổng kiểm. Mỗi pha một cổng; Three.js được giữ làm đường dự phòng cho tới khi bản native đạt tương đương.
status: accepted
owner: simulation-architecture
last_reviewed: 2026-11-20
---

# Map v2 — thế giới chân thực nhất có thể

> **Quyết định của chủ dự án (trong chat, 2026-11-20):** "Tôi muốn làm tốt nhất có thể, làm trong thời gian dài
> cx đc". Đây là **QĐ-MAPV2**: Map v2 bỏ khoá đồ hoạ của QĐ-SCOPE. Phần khoa học vẫn giữ QĐ-VIAB và chạy song
> song trên máy khác.

## 1. Mục tiêu, và "tốt nhất có thể" đo bằng gì

**Mục tiêu.** Người xem phải tin được rằng đây là một thế giới có thật. Thế giới đó vẫn phải là đúng thế giới
đang được mô phỏng: quy tắc trung thực không đổi (§6).

"Tốt nhất có thể" được đo bằng các cổng sau. Không cổng nào nới ra để một bước được tính là xong.

| Cổng | Đo bằng gì |
|---|---|
| **G-REF** — giống tham chiếu | Mỗi biome có một bộ ảnh tham chiếu thật. Ảnh chụp chuẩn của Anima được đặt cạnh ảnh tham chiếu, và chủ dự án duyệt bằng mắt. Duyệt bằng mắt là bắt buộc; nó là thước đo, không phải thủ tục |
| **G-SHAPE** — địa hình thật | Mạng sông dendritic, tức hình cành cây: số Strahler, mật độ lưới sông và tỉ lệ dài–diện tích Hack nằm trong khoảng của địa hình thật. Thung lũng hình chữ V ở thượng nguồn và hình chữ U hoặc đồng bằng ở hạ lưu. Không có hố nội lưu vô lý. Mỗi chỉ tiêu đo bằng script, không chỉ nhìn |
| **G-COMP** — thành phần thế giới | Mọi môi trường có vùng nhìn thấy được. Mở rộng cổng thành phần hiện có (`utils/worldComposition.ts`) |
| **G-PERF** — nhẹ | Iris Xe ở mức Thấp: tối thiểu 30 FPS, có 1.000 sinh vật, p5 được báo cáo. RTX 5060 Ti ở mức Cao: tối thiểu 60 FPS. Đo trong app desktop, không phải trong trình duyệt. Mỗi hệ thống nằm trong ngân sách mili-giây của nó (§3b) |
| **G-SIM** — mô phỏng còn đúng | Lưới 256² của mô phỏng sinh từ cùng nguồn. Kiểm bất biến sinh thái và năng lượng EU khép kín. Các thí nghiệm ghim `world_identity`, nên đổi thế giới thì cần manifest mới, và không có kết quả cũ nào bị đổi lặng lẽ |

## 2. Vì sao bản đồ hiện tại chưa chân thực

Chẩn đoán từ các ảnh chụp chuẩn ngày 2026-11-20, sau W11:

1. **Mỗi biome một màu phẳng, ranh giới cứng.** Ngoài đời, màu đất biến đổi liên tục theo độ ẩm, độ dốc,
   hướng nắng và loại đá, và có chi tiết ở nhiều tỉ lệ.
2. **Hình dạng địa hình chưa thật.** Thiếu thung lũng do xói mòn, đồng bằng phù sa và chân núi bồi tụ. Núi bị
   phóng đại thành gai. Lưới hiển thị thô nên nhìn gần thấy khối.
3. **Thực vật đơn điệu.** Mỗi biome một loại cây, cùng cỡ, cùng mật độ. Không có cỏ, bụi hay đá phủ mặt đất;
   không có gió.
4. **Nước là mặt phẳng tô màu.** Sông là vệt vẽ lên texture, thác là một tấm mỏng.
5. **Ánh sáng "nhựa".** Không có sương mờ theo khoảng cách, bầu trời đơn giản, mức Thấp không có bóng đổ, tone
   mapping phẳng.
6. **Tỉ lệ sa bàn.** Thế giới 200 đơn vị trông như mô hình thu nhỏ, và sinh vật ở xa chỉ là chấm.

Mục 1, 3, 4 và 5 thuộc về renderer. Mục 2 và 6 thuộc về **dữ liệu thế giới**: không shader nào làm giả được một
mạng sông thật.

## 3. Kiến trúc đích

```
World v2 generator (Rust, offline, deterministic)
   └─► pinned artifact (ADR-0010): 4096² render layers + 256² sim grid, SHA-256 in the manifest
         ├─► simulation (Bevy ECS) reads the 256² grid, as it does today
         └─► renderer reads the render layers
               ├─ native renderer (wgpu, same process as the simulation, surface in the Tauri window) — target
               └─ Three.js viewer — fallback and web demo until the native renderer reaches parity
```

- **World v2 không phụ thuộc renderer.** Viewer nào cũng hưởng lợi, nên World v2 làm trước và không bao giờ là
  công sức bỏ đi.
- **Renderer native là hướng dài hạn đã quyết từ trước (QĐ-NATIVE).** Nó bỏ được IPC theo từng tick và dùng
  được toàn bộ GPU.
- **Hiện mô phỏng chỉ dùng `bevy_ecs` 0.13, chưa có phần render của Bevy.** Có hai ngả:
  - (a) tự viết renderer bằng `wgpu`;
  - (b) nâng cả mô phỏng lên Bevy mới để dùng renderer của Bevy.

  Pha 2 đo và chọn giữa hai ngả này (ADR-0011). Một ứng viên thứ ba, Godot 4 làm viewer riêng, cũng được đo để
  chắc là ta không bỏ qua đường nhanh hơn.
- **Asset dùng chung** (texture PBR, mô hình thực vật, KTX2): chỉ CC0 từ ambientCG và Poly Haven (QĐ-ASSETS).
  Dùng được cho mọi renderer.

## 3b. Nhẹ mà đẹp: ngân sách khung hình và kỹ thuật

> **Yêu cầu của chủ dự án (trong chat, 2026-11-20):** "vừa nhẹ vừa đẹp, tối ưu". Đây là ràng buộc cứng cho
> mọi pha. Mỗi mili-giây GPU phải đổi được ra nhiều chất lượng nhìn thấy nhất có thể.

**Nguyên tắc.**

1. **Cái gì không đổi thì tính trước, đừng tính mỗi khung hình.**
   - Hình dạng địa hình, xói mòn, sông và khí hậu được sinh ngoại tuyến (World v2), nên không tốn gì lúc chạy.
     Đây là phần làm tăng chất lượng nhiều nhất với giá rẻ nhất.
   - Phần chiếu sáng tĩnh của địa hình được nướng sẵn vào texture: che khuất (AO), tầm nhìn bầu trời, bóng của
     núi lên thung lũng.
   - Tán xạ khí quyển dùng bảng tra tính sẵn, theo kiểu Bruneton hoặc Hillaire. Giá mỗi pixel vì thế rất nhỏ.
2. **Chi phí không đổi theo kích thước thế giới.**
   - Địa hình dùng clipmap hoặc CDLOD, có virtual texture hoặc mảng texture. Bộ nhớ và số lệnh vẽ giữ nguyên
     dù thế giới lớn bao nhiêu.
   - Mọi thứ được tải theo tile quanh camera.
3. **Vật liệu: ít lớp, chọn khôn.**
   - Mỗi pixel pha tối đa 2–4 lớp: dùng control map, rồi pha theo chiều cao.
   - Triplanar chỉ dùng ở vách dốc.
   - Một texture màu macro độ phân giải thấp tạo biến thiên ở xa.
   - Texture nén GPU (KTX2/Basis, BCn), có mipmap.
4. **Thực vật: đắt ở gần, rẻ ở xa.**
   - Dùng instancing, và việc loại bỏ vật thể khuất do GPU làm.
   - LOD theo khoảng cách, impostor bát diện ở xa, mật độ giảm dần theo khoảng cách.
   - Cỏ phủ chỉ có quanh camera.
   - Gió tính trong vertex shader.
5. **Nước giả khéo, không mô phỏng.**
   - Fresnel cộng phản chiếu bầu trời.
   - Màu nước theo độ sâu, đọc từ depth buffer.
   - Bọt bờ lấy từ trường khoảng cách tới bờ, nướng sẵn.
   - Phản chiếu theo màn hình (SSR) chỉ ở mức Cao.
6. **Vẽ ít pixel hơn rồi phóng lên.** Upscale theo thời gian (FSR, mã nguồn mở, hoặc TAAU) giúp Iris Xe vẽ ở
   độ phân giải thấp hơn mà vẫn nét.
7. **Cùng một nghệ thuật, nhiều mức đồ hoạ.**
   - Mức Thấp: ánh sáng và bóng nướng sẵn, impostor, không có shadow map.
   - Mức Cao: bóng nhiều tầng thời gian thực, AO, sương khối.
   - Hai mức không được trông như hai trò chơi khác nhau.
8. **Đo, không đoán.**
   - Mỗi hệ thống đo bằng GPU timer query trong app desktop, trên Iris Xe và RTX.
   - Mỗi mốc báo cáo số mili-giây của từng phần so với ngân sách bên dưới.

**Ngân sách khung hình**

Đây là điểm xuất phát; P2 hiệu chỉnh lại bằng số đo.

| Hệ thống | Iris Xe, mức Thấp (33,3 ms, vẽ 720p rồi phóng lên) | RTX 5060 Ti, mức Cao (16,6 ms) |
|---|---|---|
| Địa hình | 6 ms | 3 ms |
| Thực vật | 6 ms | 4 ms |
| Nước | 3 ms | 2 ms |
| Bầu trời và khí quyển | 1,5 ms | 1 ms |
| Sinh vật (1.000) | 5 ms | 2,5 ms |
| Hậu kỳ, upscale | 4 ms | 2 ms |
| Dự phòng: CPU, mô phỏng, UI | 7,8 ms | 2,1 ms |

Một mốc vượt ngân sách phần của nó thì chưa xong. Muốn đổi ngân sách thì phải ghi lý do, đúng như quy định đã
có cho ngân sách gói JS.

**Đo "đẹp trên mỗi mili-giây".** Ở mỗi mốc, ảnh chụp chuẩn được đặt cạnh bảng tham chiếu G-REF, kèm số
mili-giây của mốc đó. Hai phương án cùng đạt G-REF thì chọn phương án nhẹ hơn.

## 4. Lộ trình theo pha

Mỗi pha có cổng ra riêng. Thời gian là ước lượng, không phải cam kết.

| Pha | Việc | Cổng ra | Ước |
|---|---|---|---|
| **P0** Tham chiếu và thước đo | Bộ ảnh tham chiếu cho từng biome: chủ dự án chọn hoặc gửi; lead tổng hợp. Danh sách góc chụp chuẩn mới. Script đo G-SHAPE. Định nghĩa đo G-PERF trong app desktop | Chủ dự án duyệt bộ tham chiếu và các cổng. **Bộ tham chiếu đã được duyệt ngày 2026-11-20:** [bảng tham chiếu G-REF](../design/2026-11-20-map-v2-reference-board.md), 53 ảnh. Script G-SHAPE và định nghĩa đo G-PERF chuyển sang thẻ WV2-D và NR-SPIKE | 1 tuần |
| **P1** World v2 | Bộ sinh mới bằng Rust, chạy ngoại tuyến:<br>• nâng địa hình kiểu kiến tạo (dãy núi, rift, đồng bằng);<br>• xói mòn thuỷ lực theo luật stream-power và xói mòn nhiệt;<br>• bồi tụ, đồng bằng phù sa, châu thổ;<br>• sông có lưu lượng, rộng và sâu theo lưu lượng;<br>• hồ bằng priority flood, thác ở điểm gãy;<br>• khí hậu theo vĩ độ, độ cao, gió chủ đạo và mưa khuất núi;<br>• biome liên tục kèm vùng chuyển tiếp;<br>• dữ liệu hang (cho tầng ngầm sau này).<br>Đầu ra là artifact ghim hash, với thứ tự sinh tất định. Có thể chạy trên grid OxideSwarm (QĐ-OXS) | G-SHAPE, G-COMP, G-SIM. Viewer Three.js hiện thế giới mới mà không tụt FPS ở mức Thấp | 3–5 tuần |
| **P2** Spike renderer và ADR-0011 | Đo ba ứng viên trên cùng một cảnh: địa hình World v2, nước, 10.000 cây, 1.000 sinh vật.<br>(a) wgpu tự viết trong cửa sổ Tauri;<br>(b) Bevy mới, kèm ước lượng chi phí nâng mô phỏng;<br>(c) Godot 4 làm viewer riêng.<br>Đo FPS và bộ nhớ trên Iris Xe và RTX, độ phức tạp tích hợp, và đóng gói trên Windows và macOS | ADR-0011 được chấp nhận. Mã spike không merge; chỉ merge ghi chú kết quả | 2–3 tuần |
| **P3** Dựng renderer | Các mốc:<br>• **R1** địa hình: LOD clipmap hoặc CDLOD, vật liệu pha theo trường liên tục (độ dốc, độ cao, độ ẩm), triplanar ở vách, pha theo chiều cao, chi tiết nhiều tỉ lệ;<br>• **R2** nước: biển có sóng, phản chiếu và bọt bờ; sông là lưới mặt nước theo lưu lượng; thác thật;<br>• **R3** khí quyển: bầu trời vật lý, sương mờ theo khoảng cách, bóng đổ nhiều tầng, chu kỳ ngày đêm;<br>• **R4** thực vật: instancing do GPU điều khiển, nhiều loài mỗi biome, cỏ phủ, gió, impostor ở xa;<br>• **R5** sinh vật: thân liền từ hình thái (chuyển thuật toán B1), LOD;<br>• **R6** hậu kỳ và mức đồ hoạ: TAA, AO, tone mapping AgX, bốn mức chất lượng | Mỗi mốc phải qua G-REF với biome liên quan, G-PERF và G-SIM | 10–16 tuần |
| **P4** Tương đương và chuyển | Renderer native thành mặc định, Three.js thành đường dự phòng. Buổi dùng thử lần hai với người ngoài | Chủ dự án chấp nhận | 2–4 tuần |

## 5. Phân công (đề xuất, chủ dự án đổi được)

- **Windows 1 (RTX):**
  - VIAB-1 xong thì làm P2 (spike), rồi P3, tức các mốc renderer.
  - Đây là máy duy nhất có GPU mạnh, và đo được cả mức Cao lẫn mức Thấp.
- **Mac:**
  - FOUNDER-LAYOUT xong thì làm P1 (bộ sinh World v2, bằng Rust).
  - Việc sinh nặng có thể đẩy lên grid OxideSwarm.
- **Dell:**
  - Làm M2 rồi tới phần khoa học: bản nháp đăng ký E3b và SEL2-P2 theo QĐ-VIAB, sau khi VIAB-1 có kết quả.
  - Dell cũng đo G-PERF trên Iris Xe.
- **Lead:**
  - Làm P0 cùng chủ dự án.
  - Thẩm định độc lập mọi pha.
  - Giữ các cổng.

## 6. Ràng buộc cứng

- **Quy tắc trung thực.** Thứ gì không được mô phỏng thì phải ghi là trang trí, và có nút ẩn. Bản đồ chân thực
  hơn không được phép làm mờ ranh giới này.
- **Thí nghiệm không đổi lặng lẽ.** World v2 là một thế giới mới, có `world_identity` mới. Mọi bản đăng ký đã
  khoá vẫn tái lập được trên thế giới cũ, vì artifact cũ vẫn được giữ.
- **Đường tick không cấp phát bộ nhớ.** Năng lượng EU khép kín.
- **Asset chỉ CC0.**
- Không nới ngân sách hiệu năng để một bước được tính là xong. Mọi thay đổi ngân sách là một quyết định có lý do.
- Các quyết định QĐ-REALMS (nước, trời, đất ngầm, mỗi tầng một ADR) giữ nguyên. Dữ liệu hang trong World v2 chỉ
  chuẩn bị cho tầng ngầm; tầng ngầm thật vẫn là việc sau.

## 7. Rủi ro

| Rủi ro | Giảm thiểu |
|---|---|
| Nâng `bevy_ecs` 0.13 lên Bevy mới là một cuộc di trú lớn của mô phỏng | Pha 2 đo cả ngả (a), không cần nâng Bevy. ADR-0011 chọn dựa trên số đo |
| Tích hợp surface wgpu vào cửa sổ Tauri trên Windows và macOS | Đây là việc đầu tiên spike phải chứng minh |
| Iris Xe không chạy nổi chất lượng cao | Các mức đồ hoạ giảm dần. Mức Thấp phải đạt 30 FPS, nên mức này đơn giản hơn nhiều |
| Chất lượng hình ảnh khó kiểm tự động | Có G-REF (mắt chủ dự án) cùng các script so ảnh và đo G-SHAPE |
| Đồ hoạ tranh nguồn lực với khoa học | Chia máy (§5). Khoa học không dừng |
| Agent thiếu kinh nghiệm đồ hoạ cao cấp | Chia mốc nhỏ, có thẩm định độc lập, và có ảnh trước–sau ở mọi bước |

## 8. Không khẳng định

- Các thời gian trong §4 là ước lượng, chưa đo.
- Chưa có số đo nào cho thấy renderer native nhanh hơn hay đẹp hơn: Pha 2 tồn tại chính để đo điều đó.
- "Chân thực" ở đây là thứ được duyệt qua G-REF và đo qua G-SHAPE, không phải một tuyên bố tuyệt đối.
