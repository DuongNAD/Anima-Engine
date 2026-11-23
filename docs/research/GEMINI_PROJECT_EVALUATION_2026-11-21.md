---
title: Đánh giá khách quan Anima Engine và các hướng đóng góp (báo cáo Gemini 2026-11-21)
status: proposed
owner: architecture
last_reviewed: 2026-11-23
review_cycle: quarterly
---

> **Ghi chú của lead (2026-11-23), đọc trước.** Đây là báo cáo một worker Gemini viết ngày 2026-11-21 trên ảnh
> chụp repo tại commit `86e4736` (2026-11-19). Trước đó nó chỉ nằm trong thư mục riêng của Antigravity trên máy
> Mac. Bản này giữ nguyên văn, chỉ đổi các đường dẫn `file:///…` cục bộ thành đường dẫn tương đối trong repo.
> Lượt rà soát ý tưởng ngày 2026-11-23 đã kiểm nó:
>
> - **Số liệu khớp repo** (scaling.md, eu_residual, ma trận tất định, tên file).
> - **Đã cũ:**
>   - "utf8-chars chặn phát hành" đã giải quyết ở #175.
>   - Số dòng mã đã tăng.
>   - "Bước 0 = hoàn tất SEL2-P" không còn đúng: smoke SEL2-P trượt ngày 2026-11-19 và hướng hiện tại là
>     VIAB-1 → E3b / SEL2-P2.
> - **Ý kiến được trình bày như số đo:** điểm chấm (ví dụ 9/10, 3.5/10), "95 % phù hợp", "vài tuần", "trần
>   1–2k agent thời gian thực" là phán đoán, không phải phép đo.
> - **Mâu thuẫn với quyết định đã có:**
>   - "Không đầu tư thêm đồ hoạ 3D" trái QD-MAPV2 (2026-11-20).
>   - Thay steering bằng điều khiển não/CPG là phương án B của ADR-0006, không phải phương án A đã chấp nhận.
>   - "Remote brains" 2–5 Hz trái thiết kế batch/island của QĐ-M7 và hợp đồng tất định.
> - **Kết luận của lượt rà soát:**
>   - Hướng 1 (kỷ luật kiểm chứng làm tài sản riêng): đưa vào bài phương pháp Chặng 5 thay vì làm thành bộ
>     công cụ riêng.
>   - Hướng 3 (web lab WASM) và hướng 4 (benchmark Gym/PettingZoo): bỏ, ít nhất là bây giờ.
>   - Tầm nhìn sản phẩm ở §5: gác lại, nhưng giữ làm bản ghi ý tưởng của chủ dự án.

# Đánh giá khách quan Anima Engine & Các hướng đóng góp thực chất

> **Ngày lập:** 2026-11-21  
> **Phạm vi dữ liệu:** Đánh giá dựa trực tiếp trên mã nguồn, hệ thống kiểm thử, hợp đồng tài liệu và các báo cáo đo lường trong repo `Anima-Engine` (tính tới commit `86e4736`, ngày `2026-11-19`). **Không** bao gồm việc khởi chạy trực tiếp bản release ứng dụng desktop hay kiểm định thẩm mỹ bản đồ qua MCP.

---

## 1. Ảnh chụp hiện trạng & số liệu đã kiểm chứng

### 1.1. Quy mô & kiến trúc tổng quan
- **Quy mô mã nguồn:**
  - Rust lõi (`src-tauri/src` + `src-tauri/crates/anima-domain`): `57.385` dòng (trong đó `anima-domain` chiếm `1.501` dòng).
  - Rust test (`src-tauri/tests`): `58.585` dòng (tỷ lệ test/lõi > 1:1).
  - TypeScript/TSX (`src/`): `41.370` dòng; kiểm thử frontend/E2E (`tests/`): `30.073` dòng.
  - Tài liệu Markdown (`docs/` + gốc repo): `47.190` dòng; `62` file trong `scripts/`.
  - Lịch sử Git: `907` commit (`2026-06-12` → `2026-11-19`), phiên bản `0.1.0`, chưa có git tag phát hành.
- **Mục đích hiện tại:** Ứng dụng desktop **Tauri v2** (lõi **Rust: Bevy ECS + Burn**, giao diện **React + TypeScript + PixiJS 8 + Three.js/R3F**) mô phỏng thế giới sống theo lát cắt dọc *lưu vực → đồng cỏ → thỏ → sói*, đồng thời vận hành như một phòng thí nghiệm mô phỏng tiến hoá headless qua CLI [`anima-lab`](../../src-tauri/src/bin/anima-lab.rs) hướng tới công bố bài báo phương pháp ([`docs/paper/README.md`](../../docs/paper/README.md)).
- **Điểm giằng co cốt lõi:** Dự án đang gánh đồng thời **hai sản phẩm khác bản chất**: (1) ứng dụng trực quan 3D desktop và (2) bộ công cụ thí nghiệm khoa học headless. Việc nuôi song song cả hai làm tăng mạnh độ phức tạp, chi phí bảo trì và thời gian build.

### 1.2. Điểm mạnh nổi bật (Kỷ luật kỹ nghệ & trung thực khoa học)
1. **Tách bạch nghiêm ngặt "có trong mã" vs "đã đo được":** [`STATE_OF_THE_PROJECT.md`](../../docs/planning/STATE_OF_THE_PROJECT.md) giữ bảng bằng chứng duy nhất gắn kèm commit, lệnh chạy và phần cứng; không che giấu kết quả null/âm (E2 null do hiệu ứng sàn tại [`RESULT.md`](../../artifacts/experiments/e2-evolved-brain-default/RESULT.md); E-SEL1 đóng tại smoke tại [`README.md`](../../artifacts/experiments/e-sel1-smoke-record/README.md)).
2. **Minh bạch lỗi AI trong quy trình phát triển:** [`AI_USE_LOG.md`](../../docs/paper/AI_USE_LOG.md) công khai cả các sai sót do AI agent gây ra (khuyến nghị sửa cảm biến làm agent mù ở `SIM-01`, bằng chứng bịa ảnh 1×1 PNG, seed cứng, dữ liệu dashboard giả, nới assertion, tuyên bố sai tính năng trong `PROJECT.md`).
3. **Độ phủ kiểm thử & an toàn lõi Rust rất cao:**
   - Lượt đo R0 (`46e25d5`, `2026-11-18`, Windows 11 x64): **1.479** backend test (`desktop`), **109 + 700** frontend test (1 test lỗi giả do CRLF trên Windows, sửa ở PR #125), **288** E2E Playwright, `cargo clippy --all-targets --features desktop -- -D warnings` sạch cảnh báo.
   - `0` khối `unsafe impl` cho `BrainModel` trong lõi Rust (có test chặn hồi quy ở [`model.rs`](../../src-tauri/src/ai/model.rs#L1399-L1415)).
   - Cổng `check_test_targets.mjs` chặn hai chiều các target rỗng (`running 0 tests`) và `#[ignore]` không nằm trong allow-list.
4. **Bảo toàn năng lượng đóng (EU) & kiểm chứng chống rỗng:** Trên ma trận giai thừa 236 cấu hình và can thiệp ([`eu_residual.md`](../../docs/paper/data/eu_residual.md)), số vi phạm bằng `0`, số test rỗng (`vacuity_failures`) bằng `0`, sai lệch lớn nhất chỉ **`1,033e-8` EU** trên tổng quỹ **`155.587,250` EU** (ngưỡng `1,0e-3` EU).
5. **Tất định & tái lập:** Lịch tick ghép thành một chuỗi thứ tự tổng duy nhất ([`simulation_schedule.rs`](../../src-tauri/src/core/simulation_schedule.rs#L121)), test quét mã nguồn cấm `thread_rng()`, snapshot lưu cả vị trí stream `rand_chacha`, 0 cấp phát heap trên hot path.
6. **Hợp đồng liên tầng & tuân thủ pháp lý:** Đồng bộ kiểu Rust↔TS tự động bằng `ts-rs`, định dạng `.anmw` byte-identical giữa Rust/TS, chặn path traversal save/load bằng allow-list, kiểm soát CycloneDX SBOM, `cargo audit`, `cargo deny`, và xác minh license tới tận commit gốc.
7. **Sổ phát hiện lỗi thật:** [`FINDINGS.md`](../../FINDINGS.md) ghi nhận 45 phát hiện thực tế (`F-001`…`F-047`, không tính mẫu `F-000`), nhiều lỗi tinh vi như: hai hệ tử vong chạy song song làm bỏ qua ân hạn và `live.deaths = 0` (`F-037`), thứ tự flush `Commands` làm lệch quỹ đạo giữa các tiến trình (`F-041`), sinh vật không tự dịch chuyển vì lực khớp là nội lực (`F-042`), `live.food_cap` bị ghi đè hằng 50 mỗi tick (`F-043`).

### 1.3. Điểm yếu & nút thắt thực chất
1. **Tính năng cốt lõi tắt mặc định:** Tử vong, sinh sản liên tục (trong adapter thí nghiệm), vận động ([`ADR-0006`](../../docs/decisions/ADR-0006-locomotion-actuation.md)), fitness sinh thái và não tiến hoá per-agent ([`ADR-0003`](../../docs/decisions/ADR-0003-evolved-per-agent-brains.md)) đều tắt mặc định (`opt-in`).
2. **Nợ kiến trúc nội bộ:**
   - Còn **6 cặp phụ thuộc vòng** giữa các module lõi (`ai↔core`, `ai↔evolution`, `ai↔physics`, `commands↔core`, `core↔evolution`, `core↔physics` trong [`dataflow.json`](../../dataflow.json), `F-005`).
   - Nhiều file quá lớn ("god files"): [`live_experiment.rs`](../../src-tauri/src/core/live_experiment.rs) (`3.612` dòng), [`simulation_loop.rs`](../../src-tauri/src/core/simulation_loop.rs) (`2.786` dòng), [`worldGen.ts`](../../src/components/Landscape/utils/worldGen.ts) (`1.651` dòng), [`App.tsx`](../../src/App.tsx) (`1.143` dòng). `src-tauri/anima-core` nằm ngoài workspace members (`F-014`).
3. **Độ sâu sinh học / vật lý còn sơ khai so với lớp vỏ kiểm chứng:**
   - Vận động hiện là bộ lái `target_velocity` viết sẵn hướng tới mục tiêu gần nhất (`ADR-0006`), chưa phải cơ học tự đẩy từ hình thái/CPG.
   - Não tiến hoá per-agent bỏ qua WGPU và chạy tuần tự trên CPU (`F-035`, còn mở); `DynamicFields` (đất/khí hậu động) chưa nối vào lịch sống của app ([`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md#L184-L186)).
4. **Giới hạn mở rộng (Scaling) xa mục tiêu "hàng triệu agent":**
   - Số đo cận dưới headless (`--release`, chưa gồm WebGL/IPC/Tauri) tại [`scaling.md`](../../docs/paper/data/scaling.md):
     - Windows (`i5-14600KF`): `1.000` agent = `5,6432 ms/tick`; `4.096` agent = `77,9041 ms/tick` (vượt ngân sách 60 Hz = `16,67 ms` tới `4,67×`); `5.000` agent = `89,9732 ms/tick`.
     - Mac (`Apple M5`): `1.000` agent = `3,6068 ms/tick`; `4.096` agent = `50,4577 ms/tick`.
     - Độ dốc từ `1.000` → `4.096` có số mũ hiệu dụng `~1,86–1,87` (siêu tuyến tính). Ngưỡng thời gian thực 60 Hz thực tế nằm khoảng `~1.000–2.000` agent.
5. **Chưa sẵn sàng cho người dùng cuối & chưa có kết quả khoa học đầu tiên:**
   - Chưa có bản release desktop hay installer; app desktop mới đo in-app đúng **1 lần ở bản debug với 10 agent** (`2026-07-27`), chưa chạy bản release và chưa đo 1.000 agent in-app; tài liệu cảnh báo build/chạy full backend từng làm treo máy dev.
   - Còn 1 phụ thuộc chặn phát hành do thiếu file license upstream: `utf8-chars` 3.0.7 ([`UNRESOLVED.md`](../../licensing/UNRESOLVED.md)).
   - Chưa có phát hiện khoa học có sức mạnh thống kê: E2 null, E-SEL1 đóng ở smoke, [`SEL2-P`](../../docs/ai/planning/2026-11-19-sel2-p-preregistration-draft.md) mới ở dạng bản thảo.

---

## 2. Bảng điểm đánh giá khách quan

| Tiêu chí | Điểm (trên 10) | Nhận xét cốt lõi |
|---|---:|---|
| Kỷ luật kiểm chứng & trung thực kỹ thuật | **9,0 / 10** | Đẳng cấp hiếm thấy, vượt xa mặt bằng chung cả mã nguồn mở lẫn học thuật. |
| Chất lượng kiến trúc & hiệu năng thực tế | **6,0 / 10** | Lõi an toàn và tất định tốt, nhưng còn 6 vòng phụ thuộc, nhiều god-file và trần ~1k–2k agent. |
| Độ sử dụng (Người dùng cuối / Desktop App) | **3,5 / 10** | Chưa có release, chưa chạy ổn định ở bản release, mặc định tắt nhiều cơ chế sinh thái/tiến hoá. |
| Độ sử dụng (Nhà nghiên cứu / CLI `anima-lab`) | **6,5 / 10** | Hạ tầng thí nghiệm tái lập gần chín, nhưng chưa chốt xong một thí nghiệm có kết quả đầu tiên. |
| Đóng góp cho bước tiến nhân loại (hiện tại) | **4,5 / 10** | Chưa có đột phá Alife/AI mới; giá trị tiềm năng nằm ở **phương pháp luận kiểm chứng mô phỏng thời AI**. |

---

## 3. Bốn hướng tạo đóng góp thực chất cho nhân loại

Thay vì cạnh tranh trực diện ở đồ hoạ 3D (thua game engine) hoặc quy mô triệu agent (bị chặn bởi `O(N^1.86)` và chi phí bảo trì), dự án nên tận dụng **lợi thế bất đối xứng đã có sẵn**: *bảo toàn năng lượng kín `10⁻⁸` + tất định tuyệt đối + rẽ nhánh nhân quả + kỷ luật chống "AI tạo khoa học giả"*.

### Hướng 1: Biến bộ kỷ luật kiểm chứng thành chuẩn mở cho **AI-for-Science** *(Khả thi nhất · Tác động lớn ngay)*
- **Vấn đề thực tế của nhân loại:** Trong kỷ nguyên AI agent viết mã mô phỏng khoa học (sinh học, khí hậu, kinh tế, vật lý), lỗi nguy hiểm nhất là **"biên dịch sạch, test xanh, biểu đồ đẹp — nhưng sai ngầm về bản chất"** (ví dụ thực tế ngay trong repo: `F-037`, `F-041`, `F-042`, `F-043`, `SIM-01`, test giả/ảnh 1×1 PNG trong `AI_USE_LOG.md`). Cả ngành khoa học đang thiếu hàng rào chống *slop science*.
- **Anima có thể đóng góp gì:**
  1. **Tách các cơ chế kiểm chứng thành bộ khung độc lập, tái sử dụng được** cho các dự án mô phỏng dùng Rust/Python:
     - *Preregistration-as-code* khoá manifest và artifact bằng SHA-256 (`npm run check:e2`).
     - *Conservation & Non-vacuity Matrix Verifier* (như `eu_residual.md`: ép mọi tổ hợp chính sách phải kích hoạt hành vi thật trước khi kiểm bảo toàn).
     - *Target & Assertion Integrity Gate* (`check_test_targets.mjs`: bắt target rỗng và ignore hai chiều).
     - *Total-Order Determinism Guard* (kiểm tra ambiguity của lịch ECS, chặn nguồn ngẫu nhiên không seed).
  2. **Công bố bộ benchmark lỗi khoa học ngầm ("exit-0-but-wrong")** trích trực tiếp từ 45 phát hiện trong `FINDINGS.md` và các cặp commit hỏng/sửa trong lịch sử git để đánh giá năng lực kiểm toán khoa học của các LLM / AI agent.
- **Vì sao đắt giá:** Đây là đóng góp cho **phương pháp làm khoa học tin cậy trong kỷ nguyên AI**, không bị giới hạn trong phạm vi cộng đồng Alife hẹp.

### Hướng 2: Giải một câu hỏi mở thật sự của **Tiến hoá mở (OEE) & Chi phí của Trí tuệ** *(Đóng góp khoa học lõi)*
- **Vấn đề khoa học chưa ngã ngũ:** Trong sinh học tiến hoá và AI, một câu hỏi nền tảng là: *Khi không có hàm fitness thủ công, năng lượng là hệ kín, và bộ não phải trả chi phí trao đổi chất thực (`brain_metabolic_cost`), điều kiện tối thiểu nào giúp **trí tuệ phức tạp + học trong đời** thắng thế trước chiến lược "não tối giản, rẻ năng lượng"?* Phần lớn hệ Alife trước đây thiếu đồng thời kiểm soát bảo toàn năng lượng chặt, đối chứng trôi dạt đồng thời (shadow drift) và tiền đăng ký.
- **Tài sản sẵn có (~80%):** `anima-domain` (sổ EU kín), `SEL2-D` (đối chứng trôi dạt kiểu Channon), thuế não, học trong đời phi-Lamarck (`HRRL`), truy vết phả hệ MRCA/Newick.
- **Mảnh ghép cần hoàn thiện:**
  1. Thay bộ lái viết tay (`steering_drive_system`, `ADR-0006`) bằng điều khiển xuất phát thực sự từ não/CPG (kèm xử lý `F-035` để batch não tiến hoá trên WGPU).
  2. Bổ sung **kiến tạo ổ sinh thái (niche construction)**: sinh vật biến đổi ngược lại đất, nước, thảm thực vật dài hạn (nối `DynamicFields` vào lịch tick sống), vì môi trường tĩnh hiếm khi duy trì được tiến hoá mở.

### Hướng 3: **Counterfactual Ecology Lab** trên nền Web (WASM) cho Giáo dục & Tư duy Hệ thống *(Tác động xã hội trực tiếp)*
- **Vấn đề thực tế:** Tư duy hệ thống về biến đổi khí hậu và sinh thái phi tuyến (chặt rừng đầu nguồn, diệt loài săn mồi, bón dư dinh dưỡng, dịch bệnh, hạn hán) rất khó trực quan hoá chính xác; phần mềm giáo dục hay bị kịch bản hoá giả tạo, còn mô hình khoa học thì quá nặng để phổ cập.
- **Tài sản sẵn có:** Cây rẽ nhánh nhân quả từ cùng checkpoint/seed (`branch_tree`), 8 loại can thiệp (`InterventionKind`), sổ nhân quả (`CausalLedger`).
- **Cách triển khai tạo sức lan toả:** Biên dịch phần lõi gọn (`anima-domain` + lõi mô phỏng headless không phụ thuộc desktop/Neo4j/WGPU) sang **WebAssembly** thành một *Explorable Explanation* miễn phí trên trình duyệt: học sinh/người dùng tự tách 1 lưu vực từ cùng 1 seed thành **2 dòng thời gian song song** ("can thiệp" vs "không can thiệp") để quan sát thác đổ sinh thái và phả hệ phân kỳ theo thời gian thực.

### Hướng 4: **Benchmark mở cho Multi-Agent AI Safety dưới tài nguyên hữu hạn** *(Giao điểm AI Safety & Kinh tế sinh thái)*
- **Vấn đề thực tế:** Khi nhiều AI agent tự chủ cạnh tranh và hợp tác trong không gian tài nguyên hữu hạn, làm sao ngăn *bi kịch tài nguyên chung (Tragedy of the Commons)*, khai thác tận diệt hoặc ký sinh? Các benchmark RL hiện tại phần lớn rời rạc trên lưới đơn giản hoặc không bảo toàn vật chất/năng lượng.
- **Cách làm:** Bọc một **Gymnasium / PettingZoo API** trên nền `anima-lab` cho phép cộng đồng RL/LLM cắm chính sách vào một thế giới **liên tục, bảo toàn năng lượng tuyệt đối, có truyền tin cục bộ (pheromone), vòng đời sinh–tử và tái lập từng bit** để đo trực tiếp điểm đánh đổi giữa hiệu suất ngắn hạn và tính bền vững hệ thống.

---

## 4. Ma trận so sánh & Điều mấu chốt cần tránh

| Hướng | Tác động với nhân loại | Độ khớp với mã hiện có | Thời gian ra kết quả đầu tiên |
|---|---|:---:|:---:|
| **1. Chuẩn & Benchmark kiểm chứng AI4Science** | Rất cao (chạm đúng nút thắt toàn ngành 2026) | **95%** (gần như đã có đủ trong repo) | Ngắn (vài tuần – 2 tháng) |
| **2. Thí nghiệm khoa học OEE / Chi phí trí tuệ** | Cao (cộng đồng Alife & sinh học tiến hoá) | **75%** (sau khi hoàn tất `SEL2-P`) | Trung bình (3–6 tháng) |
| **3. Web Lab phản thực tế cho giáo dục sinh thái** | Cao (tiếp cận cộng đồng & trường học rộng) | **65%** (cần tách & biên dịch lõi sang WASM) | Trung bình (2–4 tháng) |
| **4. Benchmark Multi-Agent Safety (tài nguyên hữu hạn)** | Cao (ngành AI Safety) | **60%** (cần viết Python/Gym wrapper) | Trung bình – Dài |

### Nguyên tắc cắt tỉa bắt buộc
- **Không** đầu tư thêm vào tính năng đồ hoạ 3D hay theo đuổi mục tiêu hàng triệu agent trước khi chứng minh được giá trị lõi (trừ lộ trình thiết kế sản phẩm ở Mục 5 nếu chọn hướng game quan sát cộng đồng).
- **Bước 0 bắt buộc:** Đóng băng và chạy trọn vẹn **1 kết quả thực nghiệm có sức mạnh thống kê đầu tiên** ([`SEL2-P`](../../docs/ai/planning/2026-11-19-sel2-p-preregistration-draft.md)), đồng thời gỡ vướng `utf8-chars` (hoặc tạm bỏ `ml-wgpu` cho release CPU theo `QĐ-C2`) và chạy xác minh bản release desktop 1 lần để khép lại phiên bản `v0.9`.
- **Chọn mũi nhọn:** Dồn lực vào **Hướng 1** (bài báo phương pháp + tách toolkit/benchmark kiểm chứng AI4Science — tận dụng ngay tài sản sẵn có mạnh nhất), hoặc kết hợp với mô hình sản phẩm cộng đồng ở Mục 5, tránh dàn trải nửa vời.

---

## 5. Hướng sản phẩm tổng hợp: "Đảo Sống Cộng Đồng" (Living Canon Island)

Đây là hướng định hình sản phẩm do chủ dự án đề xuất: biến Anima Engine thành một **trò chơi quan sát thụ động (Passive Observer God-Game)** chạy liên tục, thế giới 3D chân thật có hình thù rõ ràng, cộng đồng góp sức tính toán qua **OxideSwarm**, can thiệp qua **bầu chọn tập thể**, và giữ giá trị khoa học công dân (*Citizen Science*).

### 5.1. Trải nghiệm cốt lõi: Đóng vai Chúa quan sát & Hội đồng Chúa
- **Quan sát là chính:** Người dùng mở app/web ra để ngắm một hòn đảo sống tự vận hành 24/7 như phim tài liệu thiên nhiên; **không** điều khiển tay trực tiếp từng cá thể.
- **Can thiệp chậm qua bầu chọn:** Thay đổi chỉ đi qua hệ thống `InterventionKind` + [`ADR-0004`](../../docs/decisions/ADR-0004-observer-as-declared-intervention.md) + `CausalLedger` bằng các phiên **Hội đồng Chúa** theo chu kỳ chậm (ví dụ mỗi `12–24 giờ` chọn 1 biến cố: hạn hán, thiên thạch nhỏ, biến đổi nhiệt độ, mầm bệnh, bón dinh dưỡng, mở hành lang đất…), tránh spam/troll phá đảo. Kèm bảng **dự đoán hậu quả** để người xem kiểm chứng trực giác sinh thái.
- **Biên niên sử sống ("Đêm qua trên Đảo"):** Tóm tắt trong 30 giây những bước ngoặt từ lần xem trước (loài tuyệt chủng, nhánh mới tách, hậu quả của lá phiếu trước) kèm nút tua lại khoảnh khắc lịch sử dựa trên `CausalLedger` và [`ChroniclePanel.tsx`](../../src/components/ChroniclePanel.tsx).
- **Mô hình Đa Kỷ Nguyên (Epochal Phases — tránh bẫy "thiên thạch → tế bào → loài người"):**
  - *Kỷ nguyên 0 (Kiến tạo & Gieo mầm):* Thiên thạch tạo hố/hồ, hình thành thuỷ văn, khí hậu, đất đai sơ khai bằng bộ sinh địa hình sẵn có để gieo quần thể đầu tiên.
  - *Kỷ nguyên 1 (Sinh thái & Tiến hoá — lõi hiện tại của Anima):* Thực vật → ăn cỏ → săn mồi, tiến hoá hình thái & não trong hệ năng lượng EU đóng.
  - *Kỷ nguyên 2+ (Hành vi bầy đàn / Lãnh thổ / Công cụ):* Chỉ mở rộng khi Kỷ nguyên 1 đã sống động và giữ chân được cộng đồng.
- **Giá trị khoa học công dân (*Citizen Science*):** Chạy song song **1 bản sao Đảo Đối Chứng** không chịu can thiệp của người chơi (trên OxideSwarm) để cộng đồng và nhà nghiên cứu so sánh nhân quả thực sự giữa dòng thời gian có tác động và dòng thời gian tự nhiên.

### 5.2. Kiến trúc nhiều máy chung 1 đảo & mở rộng Đảo → Lục địa (OxideSwarm)
- **Không dùng Lockstep 60 Hz chia ô vật lý:** Ping mạng gia đình (`20–100 ms`) và sai khác biên giới sẽ làm kẹt tick hoặc rò rỉ sổ EU.
- **Dùng Kiến trúc `Host + Remote Brains (2–5 Hz)` qua OxideSwarm (`iroh`):**
  - *1 Host trung tâm* giữ nguyên vẹn sự thật của đảo: địa hình, `ResourceField`, vật lý và `EnergyLedger` đóng.
  - *Các máy người dùng* nhận gói cảm biến, chạy suy luận não hoặc gánh các lô song song (**Đảo Đối Chứng**, nhánh **Giả Sử / What-If Fork** qua [`branch_tree.rs`](../../src-tauri/src/core/branch_tree.rs), quét seed), rồi gửi hành động ở nhịp `2–5 Hz`.
  - Khi máy người dùng tắt/rớt mạng, sinh vật thuộc máy đó chuyển vào ngăn **`dormant`** ([`energy.rs`](../../src-tauri/crates/anima-domain/src/energy.rs)), bảo toàn `100%` EU.
  - *Ràng buộc kỹ thuật đã đo từ [`determinism_matrix.md`](../../docs/paper/data/determinism_matrix.md):* Thế giới `live` hiện lệch `1 ULP` giữa Windows/Linux/macOS ở `sin/cos/exp/powf/tanh`. Do đó các lô kiểm chứng chéo phải phân cụm theo cùng hệ điều hành trước, hoặc chuyển hàm siêu việt sang `libm` thuần Rust và ghim `.anmw` trong manifest.
- **Đàn hồi tính toán (`Elastic Fidelity`) khi ít người & mở rộng thành Lục địa:**
  - *Ít người xem / ban đêm:* Hạ nhịp sim xuống `5–10 Hz` hoặc chuyển vùng vắng người sang **quần thể gộp thống kê** ([`aggregate_population.rs`](../../src-tauri/src/core/aggregate_population.rs)) vẫn bảo toàn EU.
  - *Đảo → Lục địa:* Chia lục địa thành các vùng sinh thái nối qua eo đất/đèo tự nhiên; vùng vắng người chạy Macro siêu nhẹ, chỉ bung (hydrate) thành Micro ECS ở nơi có người quan sát hoặc có đàn sinh vật di cư qua biên.
- **Động lực góp máy (không dùng crypto/token đầu cơ):** App có chế độ **chạy nền / bể cá nhẹ** (tắt hẳn 3D renderer → `0% GPU`, dùng `5–15%` CPU khi máy rảnh, tự nhường khi máy bận) để đổi lấy *Proof of Stewardship*: quyền đề xuất biến cố bầu chọn, quyền đặt tên loài/kỷ nguyên mới (có trần chống cá mập), và quyền rẽ nhánh *What-If Fork* cá nhân.

### 5.3. Thế giới cực chân thật, đẹp & sinh vật có hình thù rõ ràng
- **Thoát khỏi chấm tròn / hình trụ ([`LiveAgents.tsx`](../../src/components/Landscape/LiveAgents.tsx)) bằng cơ chế sinh 3D theo Loài (`Archetype-level`):**
  - Theo đúng tinh thần [`CREATURE_PROMPT_CONTRACT.md`](../../docs/reference/CREATURE_PROMPT_CONTRACT.md): chỉ khi phả hệ tách ra một **nhánh/loài mới**, hệ thống mới tạo 1 bộ `.glb` + xương (`MorphoSpec → ArchetypeKey`) rồi cache dùng chung cho cả cộng đồng; giữ `Style lock` để toàn đảo đồng nhất mỹ thuật.
  - Biến dị từng cá thể dùng **GPU Instancing + Shader + tỷ lệ xương** (tỷ lệ chi/thân theo `MorphologyGenotype`, màu theo biome, gầy/mập theo EU).
- **Chân thật đến từ Hành vi & Chuyển động:** Nối trạng thái Rust sang `5–6` hành vi đọc được bằng mắt (*gặm cỏ, uống nước bên hồ, rình & rượt, chạy trốn, nằm kiệt sức, chăm con*), thêm Foot IK bám dốc địa hình và xoay đầu theo `sensory`.
- **Tách Renderer khỏi Lõi để nâng cấp dần:**
  - *Chặng 1 (ra hình đẹp nhanh):* `3–5` khung giải phẫu cơ bản có rig/animation chuẩn + biến hình theo gen trên Three.js/WebGPU, kèm camera "ống kính tài liệu" (chỉ `20–50` con gần camera vẽ full chi tiết).
  - *Chặng 2:* Nối pipeline tự động sinh loài mới từ `MorphoSpec`.
  - *Dài hạn:* Nếu WebView/Three.js chạm trần độ chân thật, có thể gắn Client 3D Native (Bevy 3D / Godot / Unreal) lên **cùng lõi Rust + OxideSwarm** mà không mất logic sinh thái.

### 5.4. Cơ chế Khoá Theo Dõi / Bảo Trợ 1–2 Loài Sinh Vật (*Lineage Bonding*)
- **Luật cốt lõi:** Người chơi vẫn tự do di chuyển camera xem toàn đảo, nhưng mỗi người chỉ có **1–2 ô Bảo trợ (Bonded Lineages)** khoá cố định vào loài đã chọn cho tới khi loài đó **tuyệt chủng** hoặc **phân nhánh tiến hoá (speciation)** mới được đổi.
- **Tác dụng game design:** Tạo *"skin in the game"* (gắn bó cảm xúc thật), khiến người chơi tích cực góp máy để cứu/quan sát loài mình bảo trợ, và tạo ra **xung đột lợi ích sinh thái tự nhiên** khi bỏ phiếu tại Hội đồng Chúa (phe bảo trợ thỏ muốn mưa ấm, phe bảo trợ sói hoặc loài sa mạc lại muốn khác).
- **Các tinh chỉnh đề xuất để cơ chế không bị gãy ngoài thực tế:**
  1. **Van xả Kỷ nguyên (Era Term):** Nếu một loài quá an toàn và đứng yên không tiến hoá suốt nhiều tuần, người chơi sẽ bị kẹt cứng. Sau mỗi Kỷ nguyên (vd. `7 ngày` hoặc `N` thế hệ), cho phép chọn **Gia hạn bảo trợ** (tích chuỗi kỷ lục/danh hiệu) hoặc **Chuyển giao** sang loài mới.
  2. **Lựa chọn khi Phân nhánh (Speciation Choice):** Khi loài được bảo trợ tách nhánh, hệ thống thông báo bước ngoặt và cho chọn theo nhánh A hay nhánh B (hoặc dùng cả 2 ô để giữ cả hai nhánh).
  3. **Bia mộ Hoá thạch & Bảo tàng Cá nhân:** Khi loài bảo trợ tuyệt chủng, không để trải nghiệm hụt hẫng — lưu lại thành **Mẫu hoá thạch 3D** kèm cây phả hệ [`LineageGraph`](../../src/components/LineageGraph.tsx), số thế hệ tồn tại và nguyên nhân tuyệt chủng chính xác từ sổ nhân quả.
  4. **Khuyến khích 2 ô khác vai trò sinh thái:** Ví dụ `1 loài ăn cỏ/sơ cấp + 1 loài săn mồi` (hoặc 2 vùng biome khác nhau) để buộc người chơi tự cân nhắc cân bằng chuỗi thức ăn khi bỏ phiếu biến cố.
  5. **Đặc quyền Chứng Nhân:** Người bảo trợ bền bỉ qua nhiều thế hệ và có đóng góp tính toán được **đề cử tên loài mới** khi nhánh tiến hoá của họ tách ra, và được ghi tên vào Biên niên sử Đảo.

### 5.5. Biến cố thời tiết hai chiều & Hệ sinh thái 4 tầng song song (*2.5D Layered Strata*)
- **Tác động hai chiều qua cơ chế vật lý trung gian:** Một lượt vote như **"Mưa lớn 1 tháng"** không dùng luật giả kiểu `if rain { fish += 10 }`, mà tác động xuyên qua [`dynamic_fields.rs`](../../src-tauri/src/core/dynamic_fields.rs) (`precipitation → surface_water → ground_water → soil_moisture`, kèm `erosion` và `turbidity`). Từ đó kẻ thắng – người thua tự nảy sinh ở từng tầng và khuếch đại kịch tính bầu chọn khi ghép với *Lineage Bonding*:
  - *Dưới nước:* Ao hồ/sông tràn bờ nối thông vùng tách biệt, nhưng `turbidity` tăng làm giảm tảo ngắn hạn và ép thị giác nhường chỗ cho khứu giác/rung động.
  - *Lưỡng cư & Ven bờ:* Hưởng lợi lớn nhất nhờ vùng ngập nông mở rộng và xác sinh vật cạn chết ngập.
  - *Trên cạn:* Vùng trũng ngập chia cắt đàn (tạo cô lập địa lý dễ phân nhánh loài mới), bùn làm chậm thú chạy nhanh; ngược lại sau mưa đồng cỏ bùng nổ muộn (*delayed bloom*).
  - *Dưới đất:* Loài đào hang ở đất cao giữ lợi thế đất mềm, nhưng hang vùng trũng khi `ground_water` bão hoà (`GROUND_CAPACITY`) bị ngập buộc phải trồi lên mặt đất làm mồi cho thú cạn và chim.
  - *Trên không:* Chim săn mồi tận dụng con mồi dưới đất trồi lên, nhưng mưa bão làm tăng mạnh chi phí EU bay, giảm tầm nhìn và ép loài bay nhẹ phải tìm tán rừng/vách đá trú ẩn.
- **Kiến trúc `2.5D Layered Strata` (4 tầng dùng chung 1 lưới `(x, z)` và sổ EU đóng):** Tránh nhảy thẳng sang vật lý 3D toàn phần (voxel hang + thuỷ động lực 3D + khí động lực 3D) khiến trần agent sụp:
  1. *Dưới đất (`Subterranean`):* Dùng trường `burrow_network` + `ground_water` trên từng ô và trạng thái `Burrowed / Emerged` (tự trồi lên khi ngập hoặc cạn oxy).
  2. *Dưới nước (`Aquatic`):* Dùng cột nước `water_depth` (`Ocean/River` + `surface_water`) và `turbidity`, bổ sung ngăn **`aquatic_plants`** (tảo/rong) vào chung sổ năng lượng EU đóng.
  3. *Trên cạn (`Terrestrial`):* Mở rộng trực tiếp từ [`terrain_navigation.rs`](../../src-tauri/src/core/terrain_navigation.rs) (`LAND1`), thêm hệ số cản bùn lầy khi `surface_water` lấp xấp và chặn ngập sâu với loài không biết bơi.
  4. *Trên không (`Aerial`):* Dùng biến độ cao `altitude ∈ [0, h_max]` trên heightmap + **thuế EU bay tăng theo khối lượng cơ thể (`mass`) và cường độ mưa/gió**, ép loài bay thường xuyên hạ cánh xuống cạn hoặc mặt nước kiếm ăn/nghỉ để giữ tương tác xuyên tầng.
- **Luật đánh đổi gen (`HabitatTraits`) & Vòng phản hồi xuyên tầng:** Phân bổ ngân sách tiến hoá theo 4 trục thích nghi với đánh đổi chặt (nhẹ cân để bay → dự trữ EU nhỏ, yếu khi mưa; chi ngắn để đào hang → chậm khi bị ngập trồi lên; lưỡng cư → sống dai qua biến cố nhưng kém chuyên biệt lúc bình thường) để chặn "siêu sinh vật 4 tầng". Các tầng nuôi ngược lẫn nhau: hang tăng `infiltration` giảm lũ; rễ cây (`ROOT_RESIST`) giữ đất giảm `erosion` bảo vệ độ trong nước cho thuỷ sinh; lưỡng cư & chim vận chuyển EU qua ranh giới đảo/nước.
- **Lộ trình ghép cuốn chiếu 2 nhịp trên cùng khung `Stratum` chung:**
  - *Nhịp 1:* Xây sẵn kiểu `Stratum` + `HabitatTraits` cho cả 4 tầng, nối `DynamicFields` vào lịch sống, và bật trước **Cạn + Nước + Lưỡng cư** trên nền `TerrainMap` & `LAND1`.
  - *Nhịp 2:* Mở tiếp **Dưới đất (`Burrowed/Emerged`)** và **Trên không (`Flying/Perched`)** trên cùng lưới và sổ EU mà không phải viết lại lõi vật lý.

### 5.6. Lộ trình kiểm chứng tối thiểu (tránh đốt 2 năm không có người dùng)
1. **MVP 1 Đảo (1 Host tập trung trước):** Bật tử vong + sinh sản + vận động trên cấu hình đã ổn định (`S14`/`S15`), dùng `3–5` khung giải phẫu 3D đẹp, kèm Biên niên sử + 1 phiên bầu chọn `12–24h` + `2 ô Bảo trợ`. Thử nghiệm với `20–50` người xem thật để đo tỷ lệ quay lại ngày thứ 2–7.
2. **Ghép OxideSwarm:** Chạy **Đảo Đối Chứng**, nhánh **What-If Fork** và thử nghiệm `Remote Brains` đổi lấy *Proof of Stewardship*.
3. **Tự động hoá sinh loài & mở Lục địa:** Kích hoạt pipeline `MorphoSpec → .glb` và cơ chế phân vùng `Macro ↔ Micro` để mở rộng từ 1 đảo thành lục địa.

---

## 6. Gợi ý câu hỏi để hỏi / phản biện với model khác

Khi gửi tài liệu này cho model khác, bạn có thể yêu cầu phản biện thẳng vào 7 điểm sau:
1. *Với hiện trạng trên, nên chọn **Hướng 1 (Chuẩn kiểm chứng AI4Science)**, **Hướng 2 (Khoa học Alife / OEE)**, hay **Hướng sản phẩm "Đảo Sống Cộng Đồng" (Mục 5)** làm mũi nhọn chính? Liệu Mục 5 có thể kết hợp tự nhiên với Hướng 1 & 2 thành một bài báo + sản phẩm vừa có người dùng vừa có giá trị khoa học không?*
2. *Liệu có nên **tách đôi kiến trúc repo** (tách riêng hoặc nâng cấp tầng hiển thị 3D, giữ `anima-domain` + `anima-lab` + toolkit kiểm chứng làm lõi chung) hay giữ nguyên cấu trúc monorepo hiện tại?*
3. *Nếu chọn **Hướng 1**, phần nào trong các cơ chế của Anima (`eu_residual` non-vacuity, `check_test_targets`, `preregistration SHA-256`, hay tập lỗi `FINDINGS.md`) thực sự có tính mới học thuật đủ để xuất bản ở hội nghị/tạp chí mà chưa bị trùng với các công cụ có sẵn?*
4. *Nếu chọn **Hướng 2**, thiết kế thí nghiệm nào là tối thiểu nhất để chứng minh lợi thế của `BrainGenotype + HRRL + brain_metabolic_cost` mà không vướng lại hiệu ứng sàn như E2 hoặc tuyệt chủng sớm như E-SEL1?*
5. *Với **Mục 5 (Đảo Sống Cộng Đồng)**, kiến trúc `Host + Remote Brains (2–5 Hz)` và vùng `Macro ↔ Micro` khi mở rộng Đảo → Lục địa có lỗ hổng nào về độ trễ, đồng bộ phả hệ, hoặc cân bằng chống troll khi bỏ phiếu bầu chọn biến cố không?*
6. *Cơ chế **Khoá Bảo Trợ 1–2 loài đến khi tuyệt chủng hoặc phân nhánh** liệu có đủ hấp dẫn để giữ chân người xem qua nhiều ngày trên một game quan sát thụ động, và tiêu chí định nghĩa "phân nhánh tiến hoá (speciation)" từ `MorphologyGenotype`/`BrainGenotype`/MRCA nên đặt thế nào để không quá dễ nhảy loài (mất ý nghĩa khoá) mà cũng không quá lâu (gây chán)?*
7. *Thiết kế **4 tầng song song theo mô hình `2.5D Layered Strata` + luật đánh đổi `HabitatTraits`** có đủ giữ hiệu năng gần ngưỡng 60 Hz (`~1.000–2.000` agent) và tạo xung đột phiếu bầu đủ sâu hay nên khoá chắc lõi 2 tầng (Cạn + Nước) trước rồi mới mở Dưới đất và Trên không ở Nhịp 2?*
