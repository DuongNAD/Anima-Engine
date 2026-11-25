---
title: Phân tích khoảng trống giữa Anima và một thế giới thật (thời tiết, Trái Đất, sự sống, hành vi)
status: proposed
owner: architecture
last_reviewed: 2026-11-26
review_cycle: quarterly
---

# Phân tích khoảng trống giữa Anima và một thế giới thật

> **Phạm vi và tính chất tài liệu.** Đây là **bản đối chiếu có nguồn**, viết theo yêu cầu của chủ dự án
> đêm 2026-11-26: *"thêm cả thời tiết mưa bão nữa, bạn nghiên cứu thật kĩ xem cần thêm những gì nữa
> những gì của 1 thế giới thật"*. Nó liệt kê một thế giới thật gồm những thành phần nào (theo các khung
> tham chiếu của khoa học Hệ Trái Đất và sinh thái học), Anima đã có, đã thiết kế hay còn thiếu từng thành
> phần, rồi đề xuất thứ tự làm và các quyết định của chủ dự án. Nó **không** phải ADR, hợp đồng hay kế
> hoạch đã duyệt. Trạng thái đo được của dự án nằm duy nhất ở
> [`STATE_OF_THE_PROJECT.md`](../planning/STATE_OF_THE_PROJECT.md) §1; quyết định của chủ dự án nằm ở
> [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md). Thiết kế thời tiết và bão đi kèm là thẻ `WX-D`
> (`docs/ai/design/2026-11-26-weather-storms-design.md`, PR #304, `status: proposed`).

---

## 0. Câu hỏi, quy ước bằng chứng và tóm tắt

### Câu hỏi

1. Một thế giới tự nhiên (không có con người) gồm những thành phần nào mà một mô phỏng tiến hoá muốn
   "giống thật" phải tính tới?
2. Anima hôm nay có gì trong mã, đã quyết hoặc đã thiết kế gì, và còn thiếu gì?
3. Sau World v2 lớn (`QD-WV2-BIG`, `QD-WV2-D2`), nên làm gì trước, cái gì phụ thuộc cái gì, và chủ dự án
   cần quyết điều gì?

### Quy ước phân loại bằng chứng

| Nhãn | Nghĩa trong tài liệu này |
|---|---|
| **Đã kiểm** | Đọc trực tiếp từ mã nguồn và tài liệu của repo ngày `2026-11-26`, tại thân cây `feat/ecological-selection` commit `38ea4621`, hoặc từ nhánh được ghi rõ. Mỗi dòng có `file:line`. |
| **Nguồn ngoài** | Bài báo hoặc dữ liệu đã kiểm DOI qua Crossref ngày `2026-11-26`, hoặc trang của tổ chức khoa học đã mở được. Khi chỉ đọc được bản tóm tắt hay trích đoạn của nguồn thứ cấp, dòng đó ghi rõ. |
| **Suy luận** | Suy ra cho Anima từ các nguồn trên. Phải kiểm bằng thí nghiệm hoặc benchmark trước khi thành ADR. |
| **chưa xác minh** | Con số chưa có nguồn đã kiểm. Không được dùng làm ngưỡng cổng. |

### Tóm tắt: mười khoảng trống quan trọng nhất (`Suy luận`)

| # | Khoảng trống | Ưu tiên | Vì sao đứng ở đây |
|---|---|---|---|
| 1 | **Một luật thời gian và đơn vị nhiệt độ thống nhất** (nhịp ngày/năm, `4,5` ngày/năm, °C) | P0 | Mọi thứ theo mùa, theo ngày và theo nhiệt đều đọc hai đồng hồ đang lệch nhau (§1.3). Không quyết thì thời tiết, phenology và điều nhiệt đều sai một cách khó thấy. |
| 2 | **Lớp khí hậu–nước sống được lên lịch và khớp vĩ độ `11°N`** | P0 | `DynamicFields` có ngân sách nước bảo toàn nhưng **không được lên lịch**, và `step_climate` còn dùng gradient vĩ độ theo hàng, trái với World v2 một vĩ độ cố định (§1.2). Thẻ `V1` đã thiết kế. |
| 3 | **Thời tiết ngẫu nhiên, giông và bão** (`WX-D`) | P0 | Yêu cầu trực tiếp của chủ dự án; là nguồn mưa cho lũ (`W1`/`W3`) và sét cho cháy rừng. Hôm nay thời tiết chỉ là lựa chọn hiển thị. |
| 4 | **Sinh lý nhiệt đọc nhiệt độ môi trường** (ectotherm/endotherm, mất nhiệt theo môi trường qua `total_cost`, torpor) | P1 | Thân nhiệt hôm nay hồi về `37 °C` theo mục tiêu, không theo môi trường; kẹp `[30, 45] °C`. Không có điều này thì thời tiết không tạo áp lực chọn lọc. |
| 5 | **Ngày/đêm trong mô phỏng** (ánh sáng cho thực vật, tầm nhìn, nhịp hoạt động, nghỉ) | P1 | Ngày/đêm hôm nay chỉ ở renderer. Nhịp ngày đêm là trục ngách thời gian lớn nhất của động vật thật. |
| 6 | **Chế độ xáo trộn: cháy, lũ, hạn, gió đổ** (`W1`–`W3`, đã thiết kế, đã duyệt cổng) | P1 | Xáo trộn tạo diễn thế và đa dạng; `QD-WCHG` đã duyệt sáu cổng, chỉ thiếu nguồn mưa và sét. |
| 7 | **Bệnh và ký sinh lây truyền** | P1 | Ký sinh là chiến lược tiêu thụ phổ biến nhất; giả thuyết Red Queen nối ký sinh với sinh sản hữu tính mà `ADR-0013` vừa chọn. Hôm nay chỉ có một forcing rút đều năng lượng. |
| 8 | **Phân giải, nấm và vòng dinh dưỡng đất khép kín** (`V4`, `V5`, `E5`) | P1 | Đã thiết kế; là chỗ năng lượng và dinh dưỡng quay lại thực vật dưới luật `QD-ECOSCALE`. |
| 9 | **Cộng sinh qua động vật: thụ phấn và phát tán hạt** | P2 | `ADR-0015` có `pollen_radius` và `dispersal_radius` kiểu phi sinh học; thụ phấn bởi động vật và ăn quả phát tán hạt cần `INVERT-D` và `AERO-D`. |
| 10 | **Chăm sóc con và chiến lược vòng đời** (đầu tư sau sinh, tuổi trưởng thành, già) | P2 | `ADR-0013` đã chấp nhận giai đoạn sống và lão hoá tất định; đầu tư của bố mẹ sau sinh chưa có ở đâu. |

---

## 1. Anima hôm nay có gì trong mã (`Đã kiểm`, thân cây `38ea4621`)

### 1.1 Khung không gian, quần thể và tính tất định

| Thành phần | Bằng chứng | Ghi chú |
|---|---|---|
| Lưới mô phỏng `256²`, hộp `200 × 200` | `src-tauri/src/core/sim_rules.rs:47, 51-53` | Với World v2 `81,92 km` (`QD-WV2-D2` WB-D7), một ô sim rộng `320 m`. |
| Số cá thể | `QD-SCALE-TRIAL` (`AGENT_WORKFLOW.md:406-412`) | Khoảng `100` động vật trong giai đoạn thử, `1.000` là mục tiêu thiết kế. Chi phí đo: `~0,33 ms/tick` ở 100 và `5,64 ms` ở 1.000, theo bản ghi đó. |
| Lịch tick đơn luồng, tất định | `src-tauri/src/core/simulation_schedule.rs:121-124` | Mọi luật mới phải vào `build_tick_schedule`, không tự dựng lịch. |
| Dòng RNG tách riêng | `src-tauri/src/core/resources.rs:925-948` (slot 1–5), `aggregate_population.rs:190` (`DORMANCY_STREAM = 7`) | Các đề xuất đang tranh slot: `LIVE_DEVELOPMENT = 6` (thẻ `V0`), `PLANT_EVOLUTION = 6` và `PLANT_GENESIS = 7` (`ADR-0015`), `WORLD_DISTURBANCE = 8` (`ADR-0016`). Cần một bảng đăng ký slot trước khi thêm dòng thời tiết. |
| Luật thế giới bất biến trong một lượt chạy | `src-tauri/src/core/experiment/laws.rs:28-32, 85` | `WorldLawSet` có `baseline_energy` và `exotic_energy`; mọi luật mới là `Option`, `None` không ghi byte nào vào `canonicalize`. |
| Bản đăng ký đã đóng băng | `src-tauri/tests/prereg_e3b_manifest_tests.rs:84, 120, 580-602`; `prereg_sel2p2_manifest_tests.rs:89, 125` | `31` khoá `live.*` và `28` observable đã ghim. Khoá mới phải mặc định về hành vi cũ. |

### 1.2 Khí quyển, nước, đất

| Thành phần | Bằng chứng | Trạng thái thật |
|---|---|---|
| Khí hậu vùng của World v2 (ngoại tuyến) | `src-tauri/crates/anima-worldgen/src/climate.rs:1-4, 38-65, 76-89, 386-390` | Vĩ độ cố định `11°N`, `T0 = 27,5 °C` ở mực biển, gradient `6,3 °C/km`, mưa sườn đón gió mục tiêu `3,2 m/năm`, sàn `0,15 m/năm`, gió đất liền từ tây sang đông trong khung lưới, ELA `4.900 m`. Đây là **khí hậu trung bình năm**, không có thời tiết. |
| Hệ số dòng chảy bão theo mùa (ngoại tuyến) | `anima-worldgen/src/config.rs:54-56` | `storm_runoff` chỉ dùng khi sinh địa hình. |
| Gói mô phỏng `ANMW v3` | `anima-worldgen/src/anmw_v3.rs:105-124, 140-143` | Mang `moisture` và `temperature` chuẩn hoá; nhiệt độ ánh xạ tuyến tính từ `[-20, +40] °C`. **Không** mang lượng mưa `m/năm`. |
| Trường động `DynamicFields` | `src-tauri/src/core/dynamic_fields.rs:66-108` | Nhiệt độ không khí, mưa, nước mặt, nước ngầm, ẩm đất, nitơ, chất hữu cơ, xói mòn, độ đục; sổ cân bằng nước `:398-402`, đập `:244`, xói mòn `:472-480`. |
| …nhưng không chạy trong thế giới sống | `src-tauri/src/core/tick_capture.rs:173-180` | "nothing inserts DynamicFields as a resource and no system steps it". Thẻ `V1` sẽ lên lịch nó. |
| `step_climate` | `dynamic_fields.rs:256-307` | Mùa là sin biên độ `0,1` (`:257`); nhiệt và mưa giảm theo **khoảng cách tới mép bản đồ** như cực (`:258-271`). Mô hình này trái với World v2 một vĩ độ. Can thiệp `TemperatureDelta`/`RainfallDelta` theo vùng (`:285-302`). |
| Nhịp mùa của cỏ | `src-tauri/src/core/ecology.rs:240-273`; `environmental_systems.rs:35-55` | `seasonal_fertility = 1 + 0,5·sin(phase)`; mặc định một năm là `100 s` sim. Đây là "mùa" duy nhất đang chạy. |
| Sự kiện môi trường toàn cục cũ | `src-tauri/src/evolution/meta_ai.rs:12-18`; `core/world_systems.rs:7-35` | `ResourceDrought`, `TemperatureSpike`, `GlacialPeriod`, `ToxicDeluge`: đổi trần thức ăn và dịch mục tiêu thân nhiệt `±5 °C` cho mọi cá thể. Không có không gian, không có thời tiết. |
| Uống nước | `src-tauri/src/core/environmental_systems.rs:337-346` | Hồ là thực thể có `current_water`; uống làm giảm nó. |

### 1.3 Thời gian

| Đồng hồ | Bằng chứng | Hệ quả |
|---|---|---|
| Mùa sim mặc định: `1 năm = 100 s` | `ecology.rs:258-267` | Hồ sơ cũ, các thí nghiệm đã đóng băng chạy trên nó. |
| Hồ sơ đảo chung: `1 ngày = 20 phút`, `1 năm = 90 phút` | `ADR-0014` D-TIME-1/2 (`docs/decisions/ADR-0014-shared-island-host-and-guests.md:102-103`) | `4,5` ngày mỗi năm: ngày hạ chí luân phiên rơi vào nửa đêm và giữa trưa (round-2 review, mục 8). |
| Hồ sơ khoa học: `60 tick` mỗi ngày sim | `QD-OPENQ` (`AGENT_WORKFLOW.md:436-437`) | Hai hồ sơ được khai báo theo manifest. |
| Ngày/đêm | `src/components/Landscape/WorldShowcase.tsx:260, 351-353` | Chỉ ở frontend; backend không có từ nào về `diurnal`/`daylight` (grep `0` kết quả trong `src-tauri/src`). |

### 1.4 Sinh vật

| Thành phần | Bằng chứng | Trạng thái thật |
|---|---|---|
| Chi phí năng lượng một đường | `src-tauri/src/core/world_systems.rs:157, 174-176` | `total_cost = k_base + maintenance + activity_cost + cognition_cost`; mọi phí mới phải vào đây. |
| Thân nhiệt | `world_systems.rs:159-172`; `src-tauri/src/ai/hrrl.rs:53-54` | Sinh nhiệt từ `total_cost`, toả nhiệt theo **độ lệch so với mục tiêu `37 °C`**, không theo môi trường; kẹp `[30, 45] °C`. |
| Nước trong cơ thể | `world_systems.rs:165-166`; `environmental_systems.rs:337-346` | Mất nước theo chi phí và đổ mồ hôi; bù bằng uống và ăn. |
| Nguyên nhân chết | `src-tauri/src/core/mortality.rs:187-192` | Chỉ `Starvation` và `Dehydration`. Nhãn "Senescence" trong tracker là sai, chính mã ghi nhận điều đó (`:168-179`). |
| Giới tính | `src-tauri/src/core/components.rs:15-18` | `AgentSex` có; sinh sản hữu tính là thẻ `V3` (`ADR-0013` đã chấp nhận). |
| Kênh hoá học | `src-tauri/src/ai/pheromone.rs:10` | Lưới pheromone khuếch tán và phân rã. |
| "Ngủ đông" | `src-tauri/src/core/agent_systems.rs:210-227` | Là LOD của người quan sát, không phải sinh học (round-2 review, mục 7). |
| Mầm bệnh | `anima-domain/src/intervention.rs:137-141`; `core/live_experiment.rs:1349-1380` | `PathogenTransmission` là **forcing khai báo** rút cùng một tỉ lệ năng lượng của mọi cá thể. Không có trạng thái nhiễm, không có tiếp xúc lây. |
| Cá, chim, thú hoang, hang | `src/components/Landscape/utils/simEnvironment.ts:13-14`; `src/components/Map2/Map2Wildlife.tsx:30` | Gắn nhãn "Decoration, not simulated" (`QD-REALMS`, `C14`). |
| Thời tiết hiển thị | `src/components/Landscape/utils/weatherKind.ts:15`; `WorldWeather.tsx:33-40`; `WorldShowcase.tsx:263` | Bốn loại `clear`/`rain`/`snow`/`fog`, người dùng chọn, không nối với mô phỏng. |

---

## 2. Đã quyết, đã thiết kế, và đang thiết kế (`Đã kiểm` trong tài liệu)

### 2.1 Đã quyết hoặc đã thiết kế (trên thân cây hoặc nhánh quyết định)

| Hạng mục | Tài liệu | Bao phủ phần nào của thế giới thật |
|---|---|---|
| Vòng đời sinh vật | [`ADR-0013`](../decisions/ADR-0013-live-creature-lifecycle.md) (accepted), [`V3`](../ai/planning/2026-11-24-v3-lifecycle-spec.md) | Tuổi, `Juvenile`/`Adult`/`Senescent`, lão hoá tất định, sinh sản hữu tính bằng trao đổi chéo tương đồng, lớn lên về kích thước (sau `ADR-0019`). |
| Xác và phân giải | [`V4`](../ai/planning/2026-11-24-v4-carcasses-decomposition-spec.md) | Phân giải theo nhiệt độ và ẩm với sổ EU chính xác. |
| Đất, nước, cỏ | [`V5`](../ai/planning/2026-11-24-v5-fields-plants-feedback-spec.md), [`V1`](../ai/planning/2026-11-24-v1-dynamic-fields-schedule-spec.md) | Nước đất, dinh dưỡng, sinh khối cỏ, phản hồi của cây; lên lịch `DynamicFields`. |
| Thực vật tiến hoá | [`ADR-0015`](../decisions/ADR-0015-evolving-plants.md) (proposed), thẻ `P1–P8` | Cây và bụi cá thể, cỏ là trường quần thể, chịu hạn, chịu bóng, phòng vệ, hạt, `pollen_radius`, `dispersal_radius`, tự thụ có giá cận huyết. |
| Bề mặt thế giới thay đổi | [`ADR-0016`](../decisions/ADR-0016-changing-world-surface.md), [thẻ `W1–W7`](../ai/planning/2026-11-24-world-change-cards.md); `QD-WCHG` (nhánh `mac/decisions-1126-wchg`, `0a74d45a`) | Mực hồ theo mùa, tuyết, cháy, lũ quét, sạt lở, đứt gãy, sóng thần giới hạn, núi lửa và kỷ địa chất nướng lại ngoại tuyến. Sáu cổng `D-WCHG-1..6` đã duyệt "như khuyến nghị"; ngạt khí núi lửa và chết đuối chuyển cho `RESP-D`. |
| Năng lượng mở, ba tầng sinh vật | [`ADR-0018`](../decisions/ADR-0018-ecosystem-scales-and-cycles.md) (proposed), [thẻ `E1–E7`](../ai/planning/2026-11-24-ecosystem-cards.md) | Mặt trời vào, nhiệt ra; cá thể / bầy / trường quần thể; Kleiber; chu kỳ phụ thuộc mật độ; nhắc "SEIR-lite" nhưng chưa có thẻ riêng. |
| Giao tiếp tiến hoá | [`ADR-0017`](../decisions/ADR-0017-evolved-communication.md) (proposed), thẻ `S1–S8` | Kênh hoá học, âm thanh, thị giác; phí qua `total_cost`; nghe trộm. Âm học đầy đủ (`S7`) bị hoãn. |
| Giao diện não–thân | [`ADR-0019`](../decisions/ADR-0019-brain-body-interface.md) (accepted) | Kiến trúc não cố định `15 → 64 → 64 → 8`: **mọi giác quan môi trường mới (ánh sáng, nhiệt, mưa, gió) là một sửa đổi ADR.** |
| Môi trường sống theo bậc | `QD-REALMS` (`AGENT_WORKFLOW.md:321-324`) | Nước trước, rồi trời, rồi lòng đất, mỗi bậc một ADR. |
| World v2 lớn | `QD-WV2-BIG` (`AGENT_WORKFLOW.md:603-618`); `QD-WV2-D2` (nhánh `mac/decisions-1125-wb`, `6afa8a42`); thiết kế `WV2-D2` (nhánh `dell/wv2-d2-scale-design`, `docs/ai/design/2026-11-26-world-v2-scale-up-design.md` §4) | `81,92 km` ở `4096²` × `20 m`; chín môi trường `E1–E9` quanh `11°N` (rừng mưa sườn gió, bóng mưa khô, đồng bằng phù sa, châu thổ và rừng ngập mặn, hồ rift, cao nguyên karst, khối núi đôi, páramo và băng nhiệt đới, bờ biển đa dạng); vùng ôn đới, cực, thu đợi preset thứ hai. Lưới sim vẫn `256²`. |
| Vẻ ngoài thật hơn | `QD-WV2-LOOK` (`AGENT_WORKFLOW.md:589-602`); prototype `mac/wv2-look-proto` `49c3dc24` | Tầng mây có `uAtmoCloudCover` (mặc định `0,65`) và gió (`src/components/Landscape/materials/atmosphere.ts:46-49, 82` trên nhánh đó). Chỉ là render. |

### 2.2 Đang thiết kế đêm nay (chưa có trên nhánh nào đọc được)

| Thẻ | Nội dung được giao |
|---|---|
| `RESP-D` | Hô hấp, bơi, chết đuối, ngạt khí, dải đặc điểm cạn–nước; khung môi trường vận động: đi, bơi, lặn, bay, lượn, đào. |
| `AQUA-D` | Cá và đời sống dưới nước. |
| `INVERT-D` | Côn trùng và động vật không xương sống dưới dạng trường bang (guild field). |
| `AERO-D` | Chim và bay. |
| `FOSS-D` | Động vật đào hang và sự sống trong hang. |
| `WV2-LOOK` | Hiện thực hoá phần hiển thị. |
| `WV2-BIG-1`, `WV2-BIG-2` | Thế giới `82 km`. |
| `WX-D` | Thời tiết và bão (thẻ đi kèm tài liệu này). |

---

## 3. Một thế giới thật gồm những gì: các khung tham chiếu (`Nguồn ngoài`)

Không có một danh sách "đầy đủ" duy nhất. Tài liệu này ghép sáu khung có thẩm quyền, mỗi khung trả lời một
câu khác nhau.

| Khung | Nó liệt kê gì | Dùng cho Anima thế nào |
|---|---|---|
| **Khoa học Hệ Trái Đất** (Steffen và cs. 2020 tổng quan sự hình thành của ngành) | Hệ Trái Đất là các quyển tương tác: khí quyển, thuỷ quyển, băng quyển, thạch quyển và thổ quyển, sinh quyển. Cách chia này là quy ước chung của ngành; người viết chưa đọc toàn văn bài tổng quan nên không gán danh sách cho bài đó (chưa xác minh theo bài). | Bộ khung cấp cao nhất cho bảng khoảng trống §4. |
| **Biến khí hậu thiết yếu** của GCOS/WMO | `55` biến khí hậu thiết yếu (đếm trên trang GCOS ngày 2026-11-26), chia khí quyển, đất liền, đại dương; **sét** và **cháy** là hai biến riêng. | Danh mục kiểm cho thời tiết, nước, băng tuyết, sinh khối, lửa. |
| **Biến đa dạng sinh học thiết yếu** (Pereira và cs. 2013) | Đề xuất khung biến cần quan sát để theo dõi đa dạng sinh học (thành phần di truyền, quần thể loài, đặc điểm loài, thành phần quần xã, cấu trúc và chức năng hệ sinh thái; tên sáu lớp lấy theo trí nhớ của người viết, chưa mở bài gốc: chưa xác minh). | Danh mục kiểm cho sinh vật và quần xã. |
| **Chu trình sinh địa hoá** (Falkowski và cs. 2008) | Vi sinh vật vận hành các chu trình carbon, nitơ và các nguyên tố khác; nước là chu trình vật lý đi kèm. | Kiểm vòng vật chất khép kín (`QD-ECOSCALE`). |
| **Mô hình hệ sinh thái tổng quát Madingley** (Harfoot và cs. 2014) | Sinh vật dị dưỡng là **cohort** theo nhóm chức năng và khối cơ thể; tự dưỡng là **stock**; quá trình: ăn, trao đổi chất, lớn, sinh sản, phát tán, chết (săn, nền, đói, già); động lực: nhiệt, mưa, nước đất, số ngày sương giá, mùa của năng suất sơ cấp. Không có nước ngọt, kẻ ăn mùn bã, con người. | Chuẩn so sánh cho "tối thiểu một hệ sinh thái cơ học cần những quá trình nào"; cách gộp cohort chính là tầng bầy/trường của `ADR-0018`. |
| **Mô hình bề mặt đất và thảm thực vật toàn cầu** (Fisher & Koven 2020; CLM5, Lawrence và cs. 2019; LPJ, Sitch và cs. 2003; SPITFIRE, Thonicke và cs. 2010) | Năng lượng bề mặt, nước, carbon, thảm thực vật động, lửa với đánh lửa do sét. | Mẫu cho lớp đất–nước–thực vật–lửa và cách nối sét với cháy. |
| **Sinh thái học xáo trộn** (Turner 2010; Seidl và cs. 2017) | Tác nhân xáo trộn phi sinh học: lửa, hạn, gió, tuyết và băng; sinh học: côn trùng, mầm bệnh; chúng tương tác và khuếch đại nhau. | Danh mục kiểm cho §4.M. |
| **ODD và mô hình hoá hướng mẫu hình** (Grimm và cs. 2006, 2020; Grimm và cs. 2005) | ODD buộc khai rõ mục đích, thực thể, quá trình, thang thời gian; POM đòi mô hình tái tạo **nhiều** mẫu hình quan sát cùng lúc. | Mỗi cơ chế mới khai một mẫu hình kiểm chứng và một đối chứng âm. |

Từ sáu khung, danh mục kiểm của tài liệu này có 14 nhóm: **A** thời tiết và khí hậu, **B** ánh sáng và bầu
trời, **C** nước, **D** đất và dinh dưỡng, **E** thực vật, **F** nấm, vi sinh và phân giải, **G** bệnh và ký
sinh, **H** cộng sinh, **I** hành vi động vật, **J** giai đoạn sống và lão hoá, **K** điều nhiệt, ngủ đông và
torpor, **L** âm thanh, **M** thiên tai, **N** giả định không có con người.

---

## 4. Bảng khoảng trống

Cột "Anima": **có** = chạy trong thế giới sống; **một phần** = có mã nhưng thiếu điều cốt lõi hoặc không được
lên lịch; **đã thiết kế** = có ADR hoặc thẻ, chưa có mã; **đang thiết kế** = được giao đêm nay; **thiếu** = không
có ở đâu. Cỡ: **S** dưới một ngày agent, **M** một tới ba ngày, **L** hơn ba ngày hoặc cần ADR riêng (`Suy luận`).
Rủi ro khoa học xét năm ràng buộc của dự án: EU đóng, luật opt-in, `SEL2-P2` và `prereg_*` bit-identical,
không Lamarck, mặc định cũ.

### A. Thời tiết và khí hậu

| Thành phần | Vai trò thật | Anima | Vì sao quan trọng (tiến hoá, hiện thực) | Phụ thuộc | Cỡ | Rủi ro khoa học | Ưu tiên |
|---|---|---|---|---|---|---|---|
| Nhịp ngày đêm của nhiệt | Ở vùng nhiệt đới, biên độ nhiệt ngày–đêm có thể lớn hơn biên độ mùa (chưa xác minh, chưa có nguồn đã kiểm cho vùng Santa Marta) | thiếu (§1.3) | Tạo ngách thời gian, áp lực điều nhiệt hằng ngày | Luật thời gian (GAP-D1) | S | Thấp nếu opt-in | P0 |
| Mùa khô/mùa mưa ở `11°N` | Vùng Santa Marta: mùa khô khoảng tháng 12–4, mùa mưa khoảng 5–11, có thể có một đợt khô giữa năm ("veranillo"); ranh giới khác nhau theo nguồn (đọc qua trích đoạn nguồn thứ cấp: WWF NT0159, WeatherSpark, nghiên cứu Río Frío) | một phần: sin mùa đối xứng `ecology.rs:248-250`, `dynamic_fields.rs:257` | Mùa khô là nút cổ chai sinh tồn và là áp lực chọn lọc mạnh nhất của vùng này | GAP-D1, `V1` | M (trong `WX-D`) | Trung bình: đổi mùa sẽ đổi quỹ đạo, chỉ được opt-in | P0 |
| Thời tiết ngẫu nhiên (ngày mưa/khô, đợt mưa) | Bộ sinh thời tiết kiểu Richardson: chuỗi Markov ướt/khô, nhiệt và bức xạ có điều kiện theo ướt/khô (Richardson 1981; Wilks & Wilby 1999) | thiếu | Biến động thời tiết, không chỉ trung bình, quyết định sống còn khi nguồn lực sát ngưỡng | GAP-D1, `V1` | M | Phải có dòng RNG riêng | P0 |
| Giông đối lưu và tăng cường địa hình | Mưa địa hình (Roe 2005; Smith & Barstad 2004, đã dùng trong worldgen) | một phần: chỉ trung bình năm ngoại tuyến | Lũ quét, xói mòn, mưa không đều trong không gian | `WX-D` | M | Thấp | P0 |
| Bão nhiệt đới | Vùng Caribe Colombia ít bão: khoảng `60` sự kiện 1900–2010 (`0,55`/năm) cho cả vùng biển Caribe Colombia, theo Rey và cs. 2021 dẫn Ortiz-Royero 2012; San Andrés và La Guajira lộ diện nhiều nhất | thiếu | Hiếm nhưng gây chọn lọc tức thì: quần thể thằn lằn *Anolis scriptus* sống sót sau bão Irma và Maria khác quần thể trước bão về kích thước cơ thể, chiều dài chi tương đối và đệm ngón (Donihue và cs. 2018, tóm tắt) | `WX-D`, `W3` | M | Phải hiếm, khai báo, không mặc định | P2 |
| Gió | Gió mậu dịch, gió đất–biển, gió giật khi giông | một phần: hướng gió ngoại tuyến `climate.rs:180-181`; tầng mây prototype có gió | Phí bay, phát tán hạt, lửa lan, mất nhiệt | `WX-D`; `AERO-D` | S–M | Thấp | P1 |
| Mây, sương mù, độ ẩm | Che sáng, giảm bốc hơi, rừng mây ở sườn cao | thiếu ở sim; render có `fog` | Ánh sáng cho thực vật (`ADR-0018` D-ECO-1), nước cho rừng mây (E1 của `WV2-D2`) | `WX-D`, `ADR-0018` | S | Thấp | P1 |
| Hạn hán nhiều năm | Vùng Caribe chịu đợt hạn nặng nhất từ thập niên 1950 trong 2013–2016 (Hoyos và cs. 2019, tóm tắt) | một phần: `ResourceDrought` toàn cục `meta_ai.rs:14` | Chọn lọc cho chịu hạn (`ADR-0015`), sụp quần thể, đa dạng | `WX-D` (biến thiên giữa các năm), `V5` | S sau `WX-D` | Trung bình | P1 |

### B. Ánh sáng và bầu trời

| Thành phần | Vai trò thật | Anima | Vì sao quan trọng | Phụ thuộc | Cỡ | Rủi ro | Ưu tiên |
|---|---|---|---|---|---|---|---|
| Ngày và đêm trong mô phỏng | Ánh sáng điều khiển quang hợp, tầm nhìn, nhịp hoạt động; ngủ có thể hiểu là trạng thái bất hoạt thích nghi (Siegel 2009) | thiếu ở sim; có ở renderer | Ngách thời gian (ăn đêm/ngày), săn mồi phụ thuộc ánh sáng | GAP-D1; giác quan ánh sáng cần sửa `ADR-0019` | M | Trung bình: đầu vào não mới là sửa ADR | P1 |
| Độ dài ngày theo mùa | Ở `11°N` ngày dài khoảng `11 h 21` (đông chí) tới `12 h 39` (hạ chí), tính theo công thức thiên văn, bỏ qua khúc xạ (`Suy luận`) | thiếu | Tín hiệu mùa yếu ở nhiệt đới: phenology ở đây nên theo mưa hơn là theo độ dài ngày (`Suy luận`, Forrest & Miller-Rushing 2010 về vai trò của phenology) | GAP-D1 | S | Thấp | P2 |
| Trăng | Ánh trăng ảnh hưởng thành công săn mồi, rủi ro bị săn, kiếm ăn và dùng sinh cảnh (Kronfeld-Schor và cs. 2013, tóm tắt) | thiếu | Ngách thứ cấp | Ngày/đêm | S | Thấp | P3 |
| Sao và định hướng thiên văn | Bọ hung dùng dải Ngân Hà để định hướng (Dacke và cs. 2013) | thiếu | Không đáng ở lưới `320 m`/ô | — | — | — | P3 (không làm) |

### C. Nước

| Thành phần | Vai trò thật | Anima | Vì sao quan trọng | Phụ thuộc | Cỡ | Rủi ro | Ưu tiên |
|---|---|---|---|---|---|---|---|
| Mưa → thấm → bốc hơi → dòng chảy | Chu trình nước bề mặt; GCOS coi nước ngầm, hồ, lưu lượng sông, ẩm đất là biến thiết yếu | một phần: ngân sách bảo toàn có, không lên lịch (§1.2) | Nước là tài nguyên giới hạn thứ hai sau năng lượng; hồ hôm nay không bảo toàn | `V1` | M | Thấp nếu `live.*` mặc định tắt | P0 |
| Sông, mực hồ theo mùa, lũ | Lũ định hình môi trường sống ven sông và rừng ngập | đã thiết kế (`W1`, `W3`) | Xáo trộn, kết nối quần thể, rào cản | `V1`, `WX-D` | M | Đã có cổng `WCHG-S02/S03` | P1 |
| Nước ngầm | Nuôi dòng chảy mùa khô | một phần (`dynamic_fields.rs:85`) | Sông còn nước mùa khô quyết định nơi sống sót | `V1` | S | Thấp | P1 |
| Thuỷ triều | Caribe là biển vi triều, biên độ phần lớn `10–20 cm` (Kjerfve 1981, theo tóm tắt OSTI) | thiếu | Nhỏ so với ô `320 m`; quan trọng cho rừng ngập mặn ở thang mét, không ở thang ô | — | — | — | P3 |
| Độ mặn, cửa sông | Ranh mặn–ngọt quyết định rừng ngập mặn và cá cửa sông | thiếu (biome `Mangrove` chỉ tĩnh, `anima-worldgen/src/biomes.rs:654-664`) | Ngách cửa sông của `E4` | `AQUA-D`, `RESP-D` | M | Thấp | P2 |
| Dòng biển, oxy hoà tan | Oxy là giới hạn cho đời sống dưới nước | đang thiết kế (`RESP-D`, `AQUA-D`); hàm `DO_sat` đề xuất trong `P6 CAL-FUNCS` | Chết đuối, ngạt, nước tù | `RESP-D` | M | Thấp | P2 |

### D. Đất và dinh dưỡng

| Thành phần | Vai trò thật | Anima | Vì sao quan trọng | Phụ thuộc | Cỡ | Rủi ro | Ưu tiên |
|---|---|---|---|---|---|---|---|
| Ẩm đất, nitơ, chất hữu cơ | Phân tích gộp thí nghiệm bón: giới hạn N và P tương đương trên cạn và ở nước ngọt, bón cả hai cho đáp ứng cộng hưởng (Elser và cs. 2007) | một phần (`dynamic_fields.rs:88-90, 413`), không lên lịch | Năng suất theo không gian tạo gradient chọn lọc (E3b đo gradient năng suất) | `V1`, `V5` | M | Thấp | P1 |
| Phốt pho | Giới hạn lâu dài ở đất nhiệt đới già (`chưa xác minh` cho vùng này) | thiếu | Thêm một trục dinh dưỡng; chi phí cao, lợi ích nhỏ ở `256²` | `V5` | M | Thấp | P3 |
| Hình thành đất | Khí hậu, sinh vật, địa hình, đá mẹ, thời gian (Jenny 1941, sách) | một phần: ngoại tuyến qua biome | Đất là ký ức chậm của thế giới | `ADR-0016` Tier 3 | L | Thấp | P3 |

### E. Thực vật và vòng đời của chúng

| Thành phần | Vai trò thật | Anima | Vì sao quan trọng | Phụ thuộc | Cỡ | Rủi ro | Ưu tiên |
|---|---|---|---|---|---|---|---|
| Thực vật cá thể tiến hoá | Cây là kỹ sư hệ sinh thái, che bóng, giữ đất | đã thiết kế (`ADR-0015`) | Đồng tiến hoá thực vật–động vật ăn cỏ | `V5`, `E2/E3` | L | Đã có cổng riêng | P1 |
| Thụ phấn | Khoảng `87,5 %` thực vật có hoa thụ phấn nhờ động vật, trung bình `94 %` ở quần xã nhiệt đới (Ollerton và cs. 2011, tóm tắt) | một phần: `pollen_radius` phi sinh học (`ADR-0015:97, 120`) | Cộng sinh mạnh nhất trên cạn; tạo đồng tiến hoá | `INVERT-D`, `AERO-D` | L | Trung bình: chuyển EU giữa hai loài qua sổ | P2 |
| Phát tán hạt | Gió, nước, động vật ăn quả (Howe & Smallwood 1982) | một phần: `dispersal_radius` (`ADR-0015:97`) | Cấu trúc không gian của quần thể cây | `ADR-0015`, `AERO-D` | M | Thấp | P2 |
| Diễn thế | Các cơ chế tạo thuận, chịu đựng, ức chế (Connell & Slatyer 1977) | thiếu (nổi lên được nếu có xáo trộn + cây) | Mẫu hình POM quan trọng sau cháy và lũ | `W3`, `ADR-0015` | S (đo) | Thấp | P2 |
| Phenology (ra lá, ra hoa, kết hạt) | Thời điểm sống còn trong năm (Forrest & Miller-Rushing 2010) | thiếu | Khớp/lệch thời điểm giữa loài là áp lực chọn lọc | GAP-D1, `WX-D` | M | Trung bình (luật thời gian) | P1 |

### F. Nấm, vi sinh và phân giải

| Thành phần | Vai trò thật | Anima | Vì sao quan trọng | Phụ thuộc | Cỡ | Rủi ro | Ưu tiên |
|---|---|---|---|---|---|---|---|
| Phân giải xác và lá | Đa dạng của sinh vật phân giải ảnh hưởng tốc độ phân giải (Gessner và cs. 2010) | một phần: bể `Detritus` gộp; đã thiết kế `V4`, `E5` | Khép vòng năng lượng/dinh dưỡng; hôm nay detritus nuôi cỏ trực tiếp | `V4`, `E5` | M | Sổ EU phải chính xác | P1 |
| Nấm rễ (mycorrhiza) | Phần lớn thực vật có mạch có nấm rễ; chỉ khoảng `8 %` hoàn toàn không (Brundrett & Tedersoo 2018, tóm tắt) | thiếu | Trao đổi dinh dưỡng–carbon; có thể là đặc điểm của trường cỏ | `ADR-0015`, `E5` | M | Thấp | P3 |

### G. Bệnh, ký sinh, mầm bệnh

| Thành phần | Vai trò thật | Anima | Vì sao quan trọng | Phụ thuộc | Cỡ | Rủi ro | Ưu tiên |
|---|---|---|---|---|---|---|---|
| Lây truyền giữa cá thể | Mô hình quần thể bệnh truyền nhiễm (Anderson & May 1979) | một phần: forcing đều, không lây (§1.4); "SEIR-lite" một câu trong `ADR-0018:174` | Ký sinh là chiến lược tiêu thụ phổ biến nhất và thay đổi cấu trúc lưới thức ăn (Lafferty và cs. 2008) | `V3` | L (cần ADR) | Độc lực phải tính qua `total_cost`; dòng RNG riêng | P1 |
| Red Queen và sinh sản hữu tính | Sinh sản hữu tính như sự thích nghi chống ký sinh (Hamilton và cs. 1990) | thiếu | Chính là câu hỏi mà `ADR-0013` D1 = B mở ra; `S43` đã giữ cho động lực Red Queen giữa thú săn và mồi | `V3`, ADR bệnh | L | Trung bình | P1 |

### H. Cộng sinh và hỗ sinh

| Thành phần | Vai trò thật | Anima | Vì sao quan trọng | Phụ thuộc | Cỡ | Rủi ro | Ưu tiên |
|---|---|---|---|---|---|---|---|
| Thụ phấn, phát tán, nấm rễ | Xem E và F | một phần / thiếu | Hỗ sinh tạo đồng tiến hoá và phụ thuộc lẫn nhau | `ADR-0015`, các realm | L | Chuyển EU giữa loài phải qua sổ | P2 |
| Hợp tác trong loài | Bầy đàn, cảnh giới chung | thiếu (có thể nổi lên từ não + tín hiệu) | Cần chọn lọc nhóm hoặc thân tộc; đo bằng đối chứng | `ADR-0017` | S (đo) | Không gắn nhãn khi chưa có bằng chứng | P2 |

### I. Hành vi động vật

| Thành phần | Vai trò thật | Anima | Vì sao quan trọng | Phụ thuộc | Cỡ | Rủi ro | Ưu tiên |
|---|---|---|---|---|---|---|---|
| Lãnh thổ | Nhiều định nghĩa khác nhau trong nghiên cứu động vật có xương sống (Maher & Lott 1995) | thiếu (không có nhớ không gian) | Phân bố đều, cạnh tranh trong loài | Bộ nhớ trong não (`ADR-0003`) | L | Không được viết sẵn hành vi | P2 |
| Di cư theo mùa | Bài tổng quan "What is migration?" phân biệt các nghĩa của di cư (Dingle & Drake 2007) | thiếu | Ở `11°N` di cư theo độ cao (núi–đồng bằng theo mùa khô) hợp lý hơn di cư vĩ độ (`Suy luận`) | `WX-D` mùa khô, không gian `82 km` | M | Thấp | P2 |
| Nhóm xã hội | Bầy, đàn, cảnh giới | thiếu | Như hợp tác ở H | `ADR-0017` | M | Thấp | P2 |
| Giao tiếp | Tín hiệu hoá học, âm thanh, thị giác | đã thiết kế (`ADR-0017`) | Nghe trộm, chọn bạn tình | `S1–S8` | — | Đã có cổng | P1 (theo `ADR-0017`) |
| Chăm sóc con | Đầu tư sau sinh định hình chiến lược vòng đời | thiếu | Đánh đổi số con / chất lượng con; hiện chỉ có đầu tư lúc sinh (`ADR-0013:25`) | `V3` | M | Truyền EU cha mẹ → con qua sổ; không Lamarck (chỉ năng lượng, không trọng số học được) | P2 |
| Ngủ và nghỉ | Bất hoạt thích nghi (Siegel 2009) | thiếu | Tiết kiệm năng lượng, tránh thú săn | Ngày/đêm | S | Không viết sẵn; để não chọn | P2 |

### J. Giai đoạn sống, lão hoá, biến thái

| Thành phần | Vai trò thật | Anima | Vì sao quan trọng | Phụ thuộc | Cỡ | Rủi ro | Ưu tiên |
|---|---|---|---|---|---|---|---|
| Non, trưởng thành, già | Lão hoá tiến hoá do pleiotropy đối kháng (Williams 1957) và soma dùng một lần (Kirkwood 1977) | đã thiết kế (`ADR-0013` D2 = A, `V3`) | Chiến lược vòng đời, thời gian thế hệ | `V2`, `V3` | M | Đã có cổng `LC-S0x` | P1 |
| Biến thái | Vòng đời phức tạp tách ngách ấu trùng và trưởng thành (Moran 1994) | đã thiết kế một phần: ấu trùng là trường (`ADR-0018` §3.2) | Hai ngách trong một bộ gen | `INVERT-D`, `AQUA-D` | L | Không phát triển sau sinh ngoài lớn lên (luật round-2) | P3 |

### K. Điều nhiệt, ngủ đông, torpor

| Thành phần | Vai trò thật | Anima | Vì sao quan trọng | Phụ thuộc | Cỡ | Rủi ro | Ưu tiên |
|---|---|---|---|---|---|---|---|
| Trao đổi nhiệt với môi trường | Ngách cơ học từ cân bằng nhiệt và nước (Kearney & Porter 2009); trao đổi chất theo khối và nhiệt (Gillooly và cs. 2001; Brown và cs. 2004) | một phần: thân nhiệt có, môi trường không được đọc (§1.4) | Không có điều này thì mưa lạnh, gió, nắng không tạo chọn lọc | GAP-D2, `V1` | M | Mọi phí qua `total_cost` | P1 |
| Ectotherm và đường cong hiệu năng nhiệt | Hiệu năng phụ thuộc thân nhiệt (Huey & Kingsolver 1989) | thiếu (kẹp `[30, 45] °C`) | Mở trục tiến hoá ecto/endo | GAP-D2 | M | Mở kẹp chỉ dưới luật opt-in | P2 |
| Torpor và ngủ đông | Giảm trao đổi chất và thân nhiệt có kiểm soát (Geiser 2004) | thiếu (dormant là LOD) | Sống qua mùa khô/lạnh | GAP-D2, round-2 quyết định 6 | M | Không được trừ phí bên ngoài `total_cost` | P2 |

### L. Âm thanh

| Thành phần | Vai trò thật | Anima | Vì sao quan trọng | Phụ thuộc | Cỡ | Rủi ro | Ưu tiên |
|---|---|---|---|---|---|---|---|
| Lan truyền âm và cảnh quan âm | Môi trường chọn lọc âm thanh của chim (Morton 1975); sinh thái cảnh quan âm (Pijanowski và cs. 2011) | đã thiết kế một phần (`ADR-0017` kênh âm, `S7` hoãn); frontend có âm nền gió theo thời tiết (`WorldShowcase.tsx:417-439`) | Mưa và gió che lấp tín hiệu (`Suy luận`), một nối giữa `WX-D` và `ADR-0017` | `ADR-0017`, `WX-D` | M | Thấp | P2 |

### M. Thiên tai và xáo trộn

| Thành phần | Vai trò thật | Anima | Vì sao quan trọng | Phụ thuộc | Cỡ | Rủi ro | Ưu tiên |
|---|---|---|---|---|---|---|---|
| Cháy rừng | Lửa như "động vật ăn cỏ" toàn cầu định hình hệ sinh thái dễ cháy (Bond & Keeley 2005); mô hình toàn cầu đánh lửa bằng sét (Thonicke và cs. 2010) | đã thiết kế (`W3`); nguồn đánh lửa chưa có | Chọn lọc chịu lửa, diễn thế | `W2`, `WX-D` (sét) | M | `Plants → Detritus` dưới EU đóng | P1 |
| Sét | GCOS coi sét là biến thiết yếu; khoảng `44` tia/giây toàn cầu, tỉ lệ đất/biển khoảng `10:1` (Christian và cs. 2003, theo tóm tắt NASA); hồ Maracaibo cách vùng Santa Marta vài trăm km là điểm nóng sét lớn nhất Trái Đất (Albrecht và cs. 2016, theo tóm tắt NASA) | thiếu | Nguồn đánh lửa tự nhiên; tử vong trực tiếp hiếm | `WX-D` | S | Dòng RNG riêng | P1 |
| Lũ quét, sạt lở, gió đổ | Tác nhân xáo trộn chính (Seidl và cs. 2017); bão gây hiệu ứng thấy được và không thấy được lên rừng (Lugo 2008) | đã thiết kế (`W3`) | Mở ngách, rào cản | `W2`, `WX-D` | M | Đã có cổng | P1 |
| Động đất, núi lửa, sóng thần | Địa chất | đã thiết kế (`W3`, `W5`, `D-WCHG-3`) | Phân ly địa lý (`W7`) | `W4`–`W6` | L | Đã có cổng | P2 |
| Xáo trộn trung gian và đa dạng | Giả thuyết xáo trộn trung gian (Connell 1978) bị chỉ trích mạnh (Fox 2013) | — | Chỉ được dùng làm câu hỏi đo, không làm giả định thiết kế | — | — | — | — |

### N. Giả định không có con người

| Thành phần | Vai trò thật | Anima | Ghi chú |
|---|---|---|---|
| Không nông nghiệp, không dập lửa, không săn bắt, không khí thải | Mọi khung trên có phần "con người" (GCOS có biến sử dụng tài nguyên của con người; Madingley loại trừ tác động con người) | đúng: Anima không có con người trong thế giới; khách (`ADR-0014`) chỉ điều khiển sinh vật của mình | Nên ghi thành một câu trong `EVOLUTION_EXPERIMENT_CONTRACT` hoặc ADR thời gian: "thế giới không có con người; can thiệp chỉ qua `CauseId` của host" (GAP-D9). |

---

## 5. Lưới `256²` và `100–1.000` cá thể biểu diễn trung thực được gì (`Suy luận`)

Quy tắc chung: **thứ gì có hàng nghìn cá thể mỗi km² hoặc nhỏ hơn một ô sim thì là trường hoặc bầy; chỉ
sinh vật có thể nhập vai mới là cá thể.** `ADR-0018` §3.2 đã chọn ba tầng; bảng dưới áp nó cho các khoảng
trống ở §4.

| Hiện tượng | Thang thật | Ô sim `320 m`, `~100` cá thể | Cách biểu diễn trung thực |
|---|---|---|---|
| Thời tiết vùng, mùa, đợt mưa | `10–100 km`, ngày–tuần | Lớn hơn ô: tốt | Một trạng thái thời tiết vùng nhân với bản đồ mưa địa hình tĩnh |
| Ô giông đối lưu | `1–10 km`, `30–60 phút` (`chưa xác minh`) | Vài tới vài chục ô | Vài "đĩa giông" di chuyển, không giải khí quyển |
| Vi khí hậu (bóng cây, hướng sườn, mép nước) | `1–100 m` (Maclean và cs. 2019 cho cách hạ quy mô) | Nhỏ hơn ô | Hệ số theo ô từ sidecar ngoại tuyến (`P7 OFFLINE-TERRAIN`), không theo từng cá thể |
| Côn trùng, vi sinh, nấm, sinh vật phù du | Hàng triệu cá thể | Không thể là cá thể | Trường quần thể có phân bố đặc điểm (`ADR-0018` tầng 3) |
| Bầy cá, đàn chim nhỏ | Hàng trăm cá thể | Không thể là cá thể với `~100` tổng | Siêu cá thể (`ADR-0018` tầng 2) |
| Thú săn và con mồi lớn | Hàng chục trên vài chục km² | Vừa | Cá thể đầy đủ |
| Lãnh thổ, di cư theo độ cao | `km` | Vừa với `82 km` | Cá thể, nếu não có nhớ không gian |
| Thuỷ triều `10–20 cm`, rừng ngập mặn thang mét | `m` | Nhỏ hơn nhiều | Không mô phỏng; chỉ render |

Hệ quả: với `~100` cá thể, các phép thử tiến hoá cần nhiều seed (như `E3b`: `64` seed × `3` nhánh) chứ không
cần nhiều cá thể hơn. Mỗi cơ chế mới phải giữ chi phí nhỏ so với `~0,33 ms/tick` đo ở 100 cá thể
(`QD-SCALE-TRIAL`), vì các đề xuất "4–10 % của 5,64 ms" ở round 2 thực chất là `75–185 %` của tick thử
(round-2 review, mục 1).

---

## 6. Lộ trình đề xuất sau WV2-BIG (`Suy luận`)

Chuỗi phụ thuộc, đọc từ trên xuống. Mỗi bước opt-in; tắt thì bit-identical với thân cây.

```text
WV2-BIG-1..5 (thế giới 82 km; sim vẫn 256²)
 └─ T0 TIME-LAW: một luật thời gian + đơn vị nhiệt độ (GAP-D1, GAP-D2) ── docs + tests, không đổi hành vi
     └─ V1: lên lịch DynamicFields, bỏ gradient vĩ độ theo hàng cho hồ sơ World v2
         ├─ WX-1..WX-3 (WX-D): bộ sinh thời tiết, mưa vào DynamicFields, giông, sét
         │   ├─ W1: mực hồ theo mùa, dòng chảy (mưa của WX vào)
         │   │   └─ W2 → W3: cháy (sét của WX đánh lửa), lũ quét, sạt lở
         │   └─ WX-4 + THERMO: điều nhiệt theo môi trường qua total_cost, mở kẹp cho ecto (opt-in)
         │       └─ SENSE-ADR: một sửa đổi ADR-0019 gộp mọi giác quan môi trường (ánh sáng, nhiệt, ướt, gió)
         ├─ V2 → V3 (vòng đời, hữu tính) → DISEASE-ADR (lây truyền, Red Queen)
         └─ V4 → V5 → E5 (phân giải, đất, trường phân giải)
QD-REALMS: AQUA-D → AERO-D (chim bị gió/mưa giữ đất) → FOSS-D; INVERT-D (côn trùng bị mưa/lạnh ức chế) theo đó
ADR-0015 thực vật → thụ phấn/phát tán qua động vật (sau INVERT-D, AERO-D)
```

Thứ tự này đặt luật thời gian **trước** mọi thứ vì `WX-D`, phenology, điều nhiệt, hồ theo mùa và
`W1` đều đọc nó. Nó đặt giác quan môi trường vào **một** sửa đổi `ADR-0019` thay vì nhiều lần, vì mỗi lần đổi
số đầu vào của kiến trúc `15 → 64 → 64 → 8` là một lần phá cổng tương đương.

---

## 7. Quyết định của chủ dự án (GAP-D1 … GAP-D9)

| Slot | Câu hỏi | Phương án | Khuyến nghị |
|---|---|---|---|
| **GAP-D1** | Luật thời gian: `4,5` ngày/năm làm hạ chí luân phiên nửa đêm/giữa trưa (round-2 quyết định 2) | **(a)** giữ `90 phút` và lọc phenology theo trung bình ngày; **(b)** số ngày nguyên mỗi năm trong khung `60–120 phút` đã duyệt, ví dụ `5 ngày = 100 phút` hoặc `4 ngày = 80 phút`; **(c)** tách hẳn: mùa theo đồng hồ năm, thời tiết và ngày theo đồng hồ ngày, chấp nhận lệch pha | **(b) `5 ngày = 100 phút`**: hết trôi pha, mùa khô ~2 ngày và mùa mưa ~3 ngày vẫn đọc được bằng mắt. Mọi tốc độ của trường khai theo một quy tắc nhịp duy nhất (round-2 quyết định 2a). |
| **GAP-D2** | Đơn vị nhiệt độ: trường `[0, 1]` ứng với °C nào | **(a)** dùng đúng ánh xạ của `ANMW v3`, `T = −20 + 60·t °C` (`anmw_v3.rs:140-143`), cho hồ sơ World v2; bản đồ v1 cũ không có °C; **(b)** một ánh xạ mới khai trong luật | **(a)**: nó đã có trong định dạng đã ghim; chỉ cần ghi thành luật. Kẹp thân nhiệt `[30, 45] °C` giữ nguyên trong đường cũ, chỉ mở dưới luật sinh lý nhiệt opt-in. |
| **GAP-D3** | Quy tắc chọn trường hay cá thể cho mỗi nhóm sinh vật mới | **(a)** quy tắc §5 (nhiều/nhỏ → trường hoặc bầy; nhập vai được → cá thể), ghi vào mỗi ADR realm; **(b)** quyết từng trường hợp | **(a)**, thống nhất với `ADR-0018` D-ECO-3. |
| **GAP-D4** | Thứ tự sau WV2-BIG | **(a)** chuỗi §6 (`T0` → `V1` → `WX` → `W1–W3` → `THERMO`); **(b)** realm trước (`AQUA-D`) rồi thời tiết | **(a)**: realm dưới nước và trên trời đều cần mưa, gió, oxy và nhiệt môi trường trước. |
| **GAP-D5** | Bệnh lây truyền | **(a)** viết ADR bệnh/ký sinh sau `V3` (trạng thái nhiễm cá thể, lây theo tiếp xúc trong lưới không gian, độc lực qua `total_cost`, dòng RNG riêng, opt-in); **(b)** để sau realm | **(a)**: tương tác trực tiếp với lựa chọn hữu tính của `ADR-0013`. |
| **GAP-D6** | Giác quan môi trường của não | **(a)** một sửa đổi `ADR-0019` gộp ánh sáng, nhiệt độ môi trường, ướt, gió, sau `WX-2`; **(b)** thêm dần theo từng thẻ | **(a)**; trước đó, thời tiết chỉ tác động qua phí và vật lý, không qua hành vi chọn. |
| **GAP-D7** | Chăm sóc con | **(a)** thẻ sau `V3`: đầu tư sau sinh là đặc điểm di truyền, truyền EU qua sổ, không truyền gì học được; **(b)** không làm | **(a)**, P2. |
| **GAP-D8** | Trăng, thuỷ triều, sao | **(a)** ghi rõ là không làm (P3); **(b)** làm trăng cùng ngày/đêm | **(a)**: biên độ triều Caribe `10–20 cm` và ô sim `320 m` làm thuỷ triều vô nghĩa ở thang mô phỏng. |
| **GAP-D9** | Giả định không có con người | **(a)** ghi thành một câu trong hợp đồng thí nghiệm hoặc ADR thời gian; **(b)** để ngầm | **(a)**. |

---

## 8. Điều chưa xác minh trong tài liệu này

- Ranh giới mùa ở Santa Marta: chỉ đọc qua trích đoạn kết quả tìm kiếm (trang WWF NT0159, WeatherSpark,
  bài Río Frío của Hoyos và cs. 2019 trên MDPI, đều trả `403` khi mở trực tiếp). Hướng chung thống nhất
  (khô khoảng 12–4, mưa khoảng 5–11), chi tiết thì không.
- `60` sự kiện bão 1900–2010: số thứ cấp (Rey và cs. 2021 dẫn Ortiz-Royero 2012); bài gốc sau tường phí. Câu "San Andrés và La Guajira lộ diện nhiều nhất" lấy từ tóm tắt bài gốc trên trang RePEc, đọc qua kết quả tìm kiếm.
- Số liệu sét (`44` tia/giây, `10:1`, Maracaibo `~233` tia/km²/năm): theo tóm tắt NASA và bài báo phổ biến,
  chưa đọc bảng gốc.
- Biên độ triều `10–20 cm`: theo tóm tắt OSTI của Kjerfve 1981.
- Biên độ nhiệt ngày–đêm vùng Santa Marta, thời gian sống của ô giông, giới hạn phốt pho ở vùng này.
- Sáu lớp của EBV ghi theo trí nhớ, chưa mở bài gốc (DOI đã kiểm).
- Các thẻ "đang thiết kế" ở §2.2 chưa đọc được; tài liệu này chỉ ghi phạm vi được giao.

---

## 9. Tài liệu tham khảo

DOI kiểm qua `api.crossref.org` ngày 2026-11-26.

1. Albrecht, R. I., et al. (2016). Where are the lightning hotspots on Earth? *BAMS* 97, 2051–2068. doi:10.1175/BAMS-D-14-00193.1
2. Amador, J. A. (2008). The Intra-Americas Sea low-level jet. *Ann. N.Y. Acad. Sci.* 1146, 153–188. doi:10.1196/annals.1446.012
3. Anderson, R. M., & May, R. M. (1979). Population biology of infectious diseases: Part I. *Nature* 280, 361–367. doi:10.1038/280361a0
4. Bond, W. J., & Keeley, J. E. (2005). Fire as a global "herbivore". *TREE* 20, 387–394. doi:10.1016/j.tree.2005.04.025
5. Brown, J. H., et al. (2004). Toward a metabolic theory of ecology. *Ecology* 85, 1771–1789. doi:10.1890/03-9000
6. Brundrett, M. C., & Tedersoo, L. (2018). Evolutionary history of mycorrhizal symbioses and global host plant diversity. *New Phytologist* 220, 1108–1115. doi:10.1111/nph.14976
7. Christian, H. J., et al. (2003). Global frequency and distribution of lightning as observed from space by the Optical Transient Detector. *JGR Atmospheres* 108. doi:10.1029/2002JD002347
8. Connell, J. H. (1978). Diversity in tropical rain forests and coral reefs. *Science* 199, 1302–1310. doi:10.1126/science.199.4335.1302
9. Connell, J. H., & Slatyer, R. O. (1977). Mechanisms of succession in natural communities. *Am. Nat.* 111, 1119–1144. doi:10.1086/283241
10. Dacke, M., et al. (2013). Dung beetles use the Milky Way for orientation. *Current Biology* 23, 298–300. doi:10.1016/j.cub.2012.12.034
11. Dingle, H., & Drake, V. A. (2007). What is migration? *BioScience* 57, 113–121. doi:10.1641/B570206
12. Donihue, C. M., et al. (2018). Hurricane-induced selection on the morphology of an island lizard. *Nature* 560, 88–91. doi:10.1038/s41586-018-0352-3
13. Elser, J. J., et al. (2007). Global analysis of nitrogen and phosphorus limitation of primary producers. *Ecology Letters* 10, 1135–1142. doi:10.1111/j.1461-0248.2007.01113.x
14. Falkowski, P. G., et al. (2008). The microbial engines that drive Earth's biogeochemical cycles. *Science* 320, 1034–1039. doi:10.1126/science.1153213
15. Fisher, R. A., & Koven, C. D. (2020). Perspectives on the future of land surface models. *JAMES* 12. doi:10.1029/2018MS001453
16. Forrest, J., & Miller-Rushing, A. J. (2010). Toward a synthetic understanding of the role of phenology in ecology and evolution. *Phil. Trans. R. Soc. B* 365, 3101–3112. doi:10.1098/rstb.2010.0145
17. Fox, J. W. (2013). The intermediate disturbance hypothesis should be abandoned. *TREE* 28, 86–92. doi:10.1016/j.tree.2012.08.014
18. GCOS / WMO. Essential Climate Variables. https://gcos.wmo.int/site/global-climate-observing-system-gcos/essential-climate-variables (mở ngày 2026-11-26)
19. Geiser, F. (2004). Metabolic rate and body temperature reduction during hibernation and daily torpor. *Annu. Rev. Physiol.* 66, 239–274. doi:10.1146/annurev.physiol.66.032102.115105
20. Gessner, M. O., et al. (2010). Diversity meets decomposition. *TREE* 25, 372–380. doi:10.1016/j.tree.2010.01.010
21. Gillooly, J. F., et al. (2001). Effects of size and temperature on metabolic rate. *Science* 293, 2248–2251. doi:10.1126/science.1061967
22. Grimm, V., et al. (2005). Pattern-oriented modeling of agent-based complex systems. *Science* 310, 987–991. doi:10.1126/science.1116681
23. Grimm, V., et al. (2006). A standard protocol for describing individual-based and agent-based models. *Ecol. Model.* 198, 115–126. doi:10.1016/j.ecolmodel.2006.04.023
24. Grimm, V., et al. (2020). The ODD protocol … second update. *JASSS* 23(2). doi:10.18564/jasss.4259
25. Hamilton, W. D., Axelrod, R., & Tanese, R. (1990). Sexual reproduction as an adaptation to resist parasites. *PNAS* 87, 3566–3573. doi:10.1073/pnas.87.9.3566
26. Harfoot, M. B. J., et al. (2014). Emergent global patterns of ecosystem structure and function from a mechanistic general ecosystem model. *PLoS Biology* 12, e1001841. doi:10.1371/journal.pbio.1001841
27. Howe, H. F., & Smallwood, J. (1982). Ecology of seed dispersal. *Annu. Rev. Ecol. Syst.* 13, 201–228. doi:10.1146/annurev.es.13.110182.001221
28. Hoyos, N., et al. (2019). Modeling streamflow response to persistent drought in a coastal tropical mountainous watershed, Sierra Nevada de Santa Marta, Colombia. *Water* 11(1), 94. doi:10.3390/w11010094
29. Huey, R. B., & Kingsolver, J. G. (1989). Evolution of thermal sensitivity of ectotherm performance. *TREE* 4, 131–135. doi:10.1016/0169-5347(89)90211-5
30. Jenny, H. (1941). *Factors of Soil Formation*. McGraw-Hill (sách, không DOI).
31. Kearney, M., & Porter, W. (2009). Mechanistic niche modelling. *Ecology Letters* 12, 334–350. doi:10.1111/j.1461-0248.2008.01277.x
32. Kirkwood, T. B. L. (1977). Evolution of ageing. *Nature* 270, 301–304. doi:10.1038/270301a0
33. Kjerfve, B. (1981). Tides of the Caribbean Sea. *JGR Oceans* 86, 4243–4247. doi:10.1029/JC086iC05p04243
34. Kronfeld-Schor, N., et al. (2013). Chronobiology by moonlight. *Proc. R. Soc. B* 280, 20123088. doi:10.1098/rspb.2012.3088
35. Lafferty, K. D., et al. (2008). Parasites in food webs: the ultimate missing links. *Ecology Letters* 11, 533–546. doi:10.1111/j.1461-0248.2008.01174.x
36. Lawrence, D. M., et al. (2019). The Community Land Model version 5. *JAMES* 11, 4245–4287. doi:10.1029/2018MS001583
37. Lugo, A. E. (2008). Visible and invisible effects of hurricanes on forest ecosystems. *Austral Ecology* 33, 368–398. doi:10.1111/j.1442-9993.2008.01894.x
38. Maclean, I. M. D., et al. (2019). Microclima: an R package for modelling meso- and microclimate. *Methods Ecol. Evol.* 10, 280–290. doi:10.1111/2041-210X.13093
39. Maher, C. R., & Lott, D. F. (1995). Definitions of territoriality used in the study of variation in vertebrate spacing systems. *Animal Behaviour* 49, 1581–1597. doi:10.1016/0003-3472(95)90080-2
40. Moran, N. A. (1994). Adaptation and constraint in the complex life cycles of animals. *Annu. Rev. Ecol. Syst.* 25, 573–600. doi:10.1146/annurev.es.25.110194.003041
41. Morton, E. S. (1975). Ecological sources of selection on avian sounds. *Am. Nat.* 109, 17–34. doi:10.1086/282971
42. Ollerton, J., Winfree, R., & Tarrant, S. (2011). How many flowering plants are pollinated by animals? *Oikos* 120, 321–326. doi:10.1111/j.1600-0706.2010.18644.x
43. Ortiz-Royero, J. C. (2012). Exposure of the Colombian Caribbean coast, including San Andrés Island, to tropical storms and hurricanes, 1900–2010. *Natural Hazards* 61, 815–827. doi:10.1007/s11069-011-0069-1
44. Pereira, H. M., et al. (2013). Essential biodiversity variables. *Science* 339, 277–278. doi:10.1126/science.1229931
45. Pijanowski, B. C., et al. (2011). Soundscape ecology: the science of sound in the landscape. *BioScience* 61, 203–216. doi:10.1525/bio.2011.61.3.6
46. Rey, W., et al. (2021). Hurricane flood hazard assessment for the Archipelago of San Andres, Providencia and Santa Catalina, Colombia. *Front. Mar. Sci.* 8, 766258. doi:10.3389/fmars.2021.766258
47. Richardson, C. W. (1981). Stochastic simulation of daily precipitation, temperature, and solar radiation. *Water Resour. Res.* 17, 182–190. doi:10.1029/WR017i001p00182
48. Roe, G. H. (2005). Orographic precipitation. *Annu. Rev. Earth Planet. Sci.* 33, 645–671. doi:10.1146/annurev.earth.33.092203.122541
49. Seidl, R., et al. (2017). Forest disturbances under climate change. *Nature Climate Change* 7, 395–402. doi:10.1038/nclimate3303
50. Siegel, J. M. (2009). Sleep viewed as a state of adaptive inactivity. *Nat. Rev. Neurosci.* 10, 747–753. doi:10.1038/nrn2697
51. Sitch, S., et al. (2003). Evaluation of ecosystem dynamics … in the LPJ dynamic global vegetation model. *Global Change Biology* 9, 161–185. doi:10.1046/j.1365-2486.2003.00569.x
52. Smith, R. B., & Barstad, I. (2004). A linear theory of orographic precipitation. *J. Atmos. Sci.* 61, 1377–1391. doi:10.1175/1520-0469(2004)061<1377:ALTOOP>2.0.CO;2
53. Steffen, W., et al. (2020). The emergence and evolution of Earth System Science. *Nat. Rev. Earth Environ.* 1, 54–63. doi:10.1038/s43017-019-0005-6
54. Thonicke, K., et al. (2010). The influence of vegetation, fire spread and fire behaviour on biomass burning and trace gas emissions (SPITFIRE). *Biogeosciences* 7, 1991–2011. doi:10.5194/bg-7-1991-2010
55. Turner, M. G. (2010). Disturbance and landscape dynamics in a changing world. *Ecology* 91, 2833–2849. doi:10.1890/10-0097.1
56. Wilks, D. S., & Wilby, R. L. (1999). The weather generation game: a review of stochastic weather models. *Prog. Phys. Geogr.* 23, 329–357. doi:10.1177/030913339902300302
57. Williams, G. C. (1957). Pleiotropy, natural selection, and the evolution of senescence. *Evolution* 11, 398–411. doi:10.1111/j.1558-5646.1957.tb02911.x

Nguồn nội bộ: round-2 research review và tệp prompt realism trên hub điều phối
(`RESEARCH_REVIEW_2026-11-25_round2.md`, `RESEARCH_PROMPTS_2026-11-24b_realism.md`), đọc ngày 2026-11-26.
