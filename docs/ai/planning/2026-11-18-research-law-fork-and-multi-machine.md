---
phase: planning
feature: alternate-evolution-world-lab
title: Hai báo cáo nghiên cứu về thí nghiệm rẽ luật và thế giới nhiều máy, đối chiếu với mã
description: Đọc và đối chiếu hai báo cáo Gemini Deep Research ngày 2026-11-18 (câu 6 thí nghiệm rẽ luật, câu 7 thế giới nhiều máy). Đề xuất bốn nhánh đối chứng cho luật năng lượng lạ (QĐ-AE-C, chưa quyết), ghi các khoảng trống giữa hợp đồng ER08/ER09 và mã, và một khuyến nghị cho QĐ-M7 (lô và đảo). Không quyết gì thay chủ dự án
status: active
owner: simulation-architecture
last_reviewed: 2026-11-18
---

# Thí nghiệm rẽ luật và thế giới nhiều máy: đối chiếu hai báo cáo nghiên cứu

## 1. Nguồn và độ tin cậy

Hai báo cáo do Gemini Deep Research soạn ngày 2026-11-18. Chúng trả lời câu 6 và câu 7 của bộ bảy câu hỏi
nghiên cứu (xem phụ lục của [đề xuất làm lại SEL2](2026-11-18-sel2-redesign-proposal.md#phụ-lục-prompt-đã-dùng)).
Bản gốc do chủ dự án giữ, repo không lưu. Với hai báo cáo này, cả bảy câu đã có báo cáo.

| Báo cáo | Câu hỏi | Độ tin cậy |
|---|---|---|
| *Counterfactual Evolution in Artificial Life* | Câu 6: chứng minh một luật tự nhiên đổi khác tạo ra một lịch sử tiến hoá khác | Khá. Phần phương pháp trích nguồn gốc: Blount–Lenski–Losos 2018, Chow 2004, Olson 2013, MODES (Dolson 2019, Bohm 2024), Channon 2001, Droop–Hickinbotham 2012, DISCO (Rizzo–Székely 2010), GET (Myllymäki–Mrkvička). Bảng cỡ mẫu và các xác suất coalescent trích về `unknown_url`, tức tính toán riêng của Gemini |
| *Distributed Evolving World Architectures* | Câu 7: chia một thế giới cho hai ba máy ở nhà, sau này cho người tình nguyện | Vừa. Phần cơ chế (Repast4Py, D-MASON, Factorio, BOINC, Folding@home, Wasmtime) có nguồn tốt. Các hệ số tăng tốc (2,99×, 0,09–0,34×), mọi số tick/giây và tỉ lệ bỏ sót 90 % đều trích về `unknown_url`, tức một mô hình phân tích của Gemini, **không** phải phép đo |

Cả hai báo cáo đã được đọc hết và đối chiếu với thân cây tại `46e25d5`. Số nào trích về `unknown_url` thì
không được dùng làm bằng chứng ở đây. Có một ngoại lệ đã kiểm lại được: bảng cỡ mẫu Wilcoxon–Mann–Whitney ở
§2.4 khớp với công thức Noether khi tính lại.

## 2. Câu 6 — thí nghiệm rẽ luật (track AE)

### 2.1 Báo cáo nói gì, và repo đã có gì

| Khẳng định của báo cáo | Đối chiếu với repo | Khoảng trống |
|---|---|---|
| Đơn vị lặp lại là **một thế giới**, không phải một cá thể. Gộp cá thể giữa các thế giới là giả lặp (pseudoreplication) | AE-S14 đòi ensemble nhiều seed; `run_ensemble` (`core/experiment_runner/ensemble.rs`) chạy theo seed | Không có. Hợp đồng đã đúng |
| Rẽ nhánh từ khởi nguyên (genesis twin) và rẽ từ checkpoint (analytic replay); thêm thiết kế hai pha để tách "luật" khỏi "lịch sử" bằng mô hình thành phần phương sai | `genesis_fork`, `checkpoint_fork` (`core/experiment_runner/fork.rs`); AE-S08, AE-S09 | Thiết kế hai pha (N lịch sử × M bản lặp mỗi luật) chưa có trong hợp đồng. Nó chỉ cần runner có sẵn, không cần mã mới |
| Mỗi phân hệ một luồng RNG riêng, sinh theo khoá (seed, phân hệ) và bộ đếm (tick, thực thể, lần rút), kiểu Philox hay Threefry. Có vậy thì bật luật lạ không làm lệch pha luồng đột biến | ER08 của [hợp đồng thí nghiệm](../../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) đã đòi đúng điều này ("world seed + system + entity/lineage + event"). Mã chưa làm đủ: `core/rng.rs` chỉ có hai luồng `StdRng` tuần tự (ECS và vòng tiến hoá); `evolution_pathway.rs` có miền riêng `RNG_DOMAIN` | **Khoảng trống giữa hợp đồng và mã.** Đổi sang RNG theo bộ đếm làm đổi mọi quỹ đạo, nên theo [ADR-0008](../../decisions/ADR-0008-world-past-and-time.md) nó mở một kỷ engine mới. Đây không phải việc nhỏ và không làm lúc này |
| Pha của RNG vẫn lệch dần sau khi rẽ nhánh (cá thể chết hay sinh lệch một tick), nên tương quan cặp giảm theo thời gian. Ghép cặp seed vẫn có lợi nhưng không thay được cỡ mẫu | Phép tính MDE cặp (16–24 seed) trong [đề xuất SEL2](2026-11-18-sel2-redesign-proposal.md) giả định một tương quan cặp | Xem §2.4 |
| Bốn nhánh đối chứng để luật lạ không "thắng hiển nhiên": trường ma (field chạy, hấp thụ bị kẹp về 0), giả dược tốn phí (trả chi phí, không nhận năng lượng), đẳng năng lượng (tổng năng lượng thế giới bằng baseline), thức ăn tăng (baseline thêm đúng lượng calo của luật lạ) | Đối chứng duy nhất trong hợp đồng là `exotic_energy=None`. AE-S06 ("thế giới vắng không nhận fitness miễn phí") gần với giả dược tốn phí nhưng không phải nó | **Đề xuất QĐ-AE-C** ở §2.3 |
| Một "loài" cần bốn cổng: phân cụm genotype hiệu chuẩn trên mô hình null một hốc sinh thái, phân chia dinh dưỡng thấy được trên sổ năng lượng, tồn tại khi tắt đột biến cùng khả năng xâm nhập khi hiếm theo cả hai chiều, và nếu có lai thì giảm khả năng lai | ER09 đã cấm gọi một phenotype hay một ô MAP-Elites là loài và liệt kê năm loại bằng chứng; AE-S11 là cổng | Báo cáo cho ER09 cách đo cụ thể (mô hình null của Olson 2013, chế độ sinh thái với đột biến bằng 0, xâm nhập khi hiếm). Chép vào ER09 khi làm AE-S11, không phải bây giờ |
| Độ đo: hoạt động tiến hoá QNN (Droop–Hickinbotham) thay cho Bedau–Packard trung bình; MODES đo trên nhiều độ sâu lọc; cây phát sinh chính xác lúc chạy (Phylotrack, hstrat). Nêu các bẫy đã biết: drift làm phình độ phức tạp, cấu trúc không gian làm phình đa dạng phát sinh | Có MRCA của DAG dòng dõi (OSS-072, #75). Chưa có độ đo hoạt động tiến hoá hay MODES nào | Ghi lại làm đầu vào cho phần observable của track AE |
| Thời gian chạy phải tính bằng **thế hệ** và bằng bội của cỡ quần thể hiệu dụng `Ne`. Muốn nói về tách nhánh sâu thì cần cỡ 4–10·`Ne` thế hệ | SEL2-E vừa thêm "≥ 30 thế hệ" (QĐ-SEL2b). Thế giới sống chạy 18.000 tick cho vài chục thế hệ | Xem §2.5 |

### 2.2 Điều phù hợp với ADR-0002 và không đổi gì

Báo cáo củng cố các luật đã có: MU không phải EU và có sổ riêng; năng lượng lạ chỉ tác động qua trường →
pathway có chi phí → hiệu năng → sống sót và sinh sản; `exotic_energy=None` là đường baseline. Báo cáo cũng
cảnh báo hai bẫy mà repo đã chặn: thêm năng lượng làm tăng sức chứa (đây là lý do của ER04, "mọi nguồn có
budget"), và chi phí phải thật để pathway không trở thành nâng cấp miễn phí (AE-S06).

### 2.3 Đề xuất QĐ-AE-C: bốn nhánh đối chứng cho luật năng lượng lạ

Áp cho `ReferenceEvolutionWorld`, nơi duy nhất có năng lượng lạ (adapter sống từ chối `laws.exotic_energy`).

| Nhánh | Khác baseline ở đâu | Phải cho kết quả gì |
|---|---|---|
| T0 baseline | `exotic_energy=None` | Đường tham chiếu, như hiện nay |
| T1 luật lạ | Trường MU chạy, pathway có chi phí, hấp thụ thật | Nhánh xử lý |
| T2a trường ma | Trường và sổ MU chạy đủ, hấp thụ bị kẹp về 0, chi phí 0 | **Bằng T0 tới từng checksum.** Nếu không bằng, mã của trường có tác dụng phụ (rút thêm RNG, đổi thứ tự) |
| T2b giả dược tốn phí | Như T1, nhưng năng lượng nhận về bằng 0 | Pathway bị chọn lọc loại bỏ. Nếu nó vẫn lan rộng thì nguyên nhân là thiên lệch đột biến hoặc giác quan, không phải năng lượng |
| T3 đẳng năng lượng | Nguồn chính giảm đúng bằng lượng MU thêm vào | Tách "có hai luật" khỏi "có nhiều calo hơn" khi sức chứa giữ nguyên |
| T4 thức ăn tăng | `exotic_energy=None`, nguồn chính tăng đúng bằng lượng MU của T1 | T1 so với T4 tách "luật khác" khỏi "thêm calo dưới cùng một luật" |

Một kết luận "luật lạ làm lịch sử khác" khi đó cần cả T1 khác T4 và T3 khác T0, trong khi T2b không giữ
pathway. T2a là kiểm tra an toàn chạy trước mọi thứ. Nó cũng là một test rẻ có thể thêm ngay khi có thẻ: cùng
manifest, `exotic_energy` có trường nhưng hấp thụ 0, so checksum với `None`.

**Chưa quyết.** Câu hỏi cho chủ dự án: thêm bốn nhánh này vào ER06 và ER07 của hợp đồng thí nghiệm, kèm một thẻ
cho T2a trước? Đề xuất: có, nhưng sau chuỗi SEL2. Track AE không nằm trên đường găng của chặng 4.

### 2.4 Cỡ mẫu: con số kiểm lại được, và nó nói gì cho SEL2-P

Bảng của báo cáo dùng công thức Noether cho Wilcoxon–Mann–Whitney hai mẫu độc lập, với α = 0,05 hai phía.
Tính lại với `A` của Vargha–Delaney:

| Hiệu ứng (Romano) | `A` | Cliff δ | Seed mỗi nhánh, power 80 % | Power 90 % |
|---|---|---|---|---|
| Lớn | 0,737 | 0,474 | 24 | 32 |
| Vừa | 0,665 | 0,33 | 49 | 65 |

Công thức: tổng `N = (z₁₋α/₂ + z₁₋β)² / (3·(A − 0,5)²)`, chia đôi cho mỗi nhánh. Hai hàng khớp bảng của báo
cáo.

Đây là số cho hai mẫu **độc lập**. SEL2-P ghép cặp theo seed, nên con số thật nằm giữa phép tính MDE cặp (16–24
seed cho d_z 0,6–0,9) và bảng trên, tuỳ tương quan giữa hai nhánh của một cặp. Báo cáo lưu ý tương quan đó giảm
dần sau khi rẽ nhánh. Hệ quả cho QĐ-SEL2c ("16 hay 24 seed, quyết sau SEL2-E"): SEL2-E nên báo cả **tương quan
cặp đo được** giữa các lượt cùng seed, không chỉ thời gian chạy. Không có con số đó, phép tính 16–24 seed dựa
trên một giả định chưa kiểm.

### 2.5 Thời gian chạy và giới hạn của câu hỏi SEL2

SEL2 hỏi về một **dịch chuyển tính trạng** (khối lượng cơ thể dưới khan hiếm thức ăn), không hỏi về loài.
Báo cáo xếp nguồn gốc *de novo* của một pathway và cú quét của nó ở 600–2.000 thế hệ. SEL2 thì đo dịch
chuyển trên biến dị sẵn có, không chờ một pathway mới, nên ba mươi thế hệ có thể đủ. Đây là suy luận của tài liệu
này, chưa có phép đo nào. Còn bất kỳ tuyên bố nào về tách loài, hốc sinh thái ổn định hay cây phát sinh
sâu trên thế giới sống thì cần cỡ `4·Ne` thế hệ trở lên, xa ngoài một lượt 18.000 tick. Câu "không gọi là loài"
của ER09 vì thế không chỉ là chuyện từ ngữ: ở tốc độ hiện nay, thế giới sống không chạy đủ lâu để đo được.

## 3. Câu 7 — thế giới nhiều máy (chặng 7)

### 3.1 Báo cáo nói gì

Báo cáo so bốn kiến trúc: chia không gian thành nhiều mảnh, lockstep, máy chủ có quyền cuối với máy con tính
từng vùng, và các **đảo** ghép lỏng (mỗi máy một thế giới riêng, thỉnh thoảng gửi sinh vật di cư). Kết luận:
với một thế giới 256×256 vốn đã nhỏ và nhanh, ba kiến trúc đầu chịu một rào chắn mạng ở **mỗi tick**, nên trên
Wi-Fi hay WAN chúng chậm hơn chạy một máy. Đảo không có rào chắn, không cần toán học giống nhau từng bit giữa
các hệ điều hành, và một máy tắt không làm các máy khác dừng. Với người tình nguyện, kiểm ngẫu nhiên kiểu BOINC
lọc được máy hỏng nhưng yếu trước một kẻ gian chỉ cài một "siêu sinh vật" một lần. Báo cáo đề xuất ba lớp:
cổng bất biến cho **mọi** sinh vật di cư, lặp lại thích nghi cho máy mới, và chạy lại có hồi tố khi một dòng dõi
từ máy tình nguyện quét quá một ngưỡng của đảo.

### 3.2 Đối chiếu với repo

| Báo cáo | Repo | Ghi chú |
|---|---|---|
| Lockstep hay chia vùng có rào chắn mỗi tick là lựa chọn tệ nhất cho thế giới nhỏ qua mạng gia đình | [Thiết kế 25/08](../design/2026-08-25-feature-federated-compute.md) §4 dùng đúng lockstep theo rào chắn, và §5.2 giao mỗi máy một dải không gian | [Kế hoạch 13/11](../../planning/DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md) §5 chặng 7 đã đề xuất sửa thiết kế này trước dòng mã đầu tiên (QĐ-M7): **lô** cho `Contribute`, **đảo** cho `Inhabit`. Báo cáo này đồng ý với đề xuất đó |
| Đảo không cần toán giống từng bit giữa các hệ điều hành | Checksum thế giới đã được đo là khác giữa các nền tảng | Chỉ `Contribute` dạng lô (so bằng tuyệt đối với host) và việc chạy lại để kiểm người tình nguyện mới cần QĐ-M6. `Inhabit` dạng đảo thì không |
| Sinh vật di cư đi qua một hàng đợi không chặn, được kiểm biên trước khi thả xuống | Đã có: `ShardingConfig` (vòng trái-phải), `anima-migration-v1` qua WebSocket, `AgentMigrationData` mang genotype, não và dòng dõi; trần `MAX_MIGRATION_*`; `SpawnMigrationCommand` sửa vị trí và vận tốc không hữu hạn | Vòng các dải hiện có gần với một quần đảo vòng hơn là với lockstep. Năm lớp tin cậy T1–T5 của thiết kế 25/08 là cổng bất biến báo cáo đòi |
| Bắt đầu kiểm lại nhiều với máy mới rồi giảm dần | Kế hoạch 13/11 đã có cùng ý (T5 "bắt đầu cao và giảm dần theo chuỗi kết quả đúng") | Không mới |
| **Chạy lại có hồi tố:** khi một dòng dõi gốc từ máy tình nguyện chiếm hơn khoảng 20 % một đảo tin cậy, host chạy lại kỷ nguyên gốc của nó | Chưa có | **Mới.** Nó bịt đúng lỗ mà kiểm ngẫu nhiên để hở. Nó cần chạy lại được tới từng bit, nên phụ thuộc QĐ-M6 hoặc vào lựa chọn WebAssembly ở dòng dưới |
| WebAssembly tất định (Wasmtime, chuẩn hoá NaN, tắt relaxed SIMD) là một đường thứ hai để có kết quả giống từng bit giữa các nền tảng | QĐ-M6 có một phương án khác: `sim_math` trên `libm` cùng lệnh cấm hàm toán hệ điều hành trong `clippy.toml` ([đối chiếu Burn và tất định](2026-11-18-research-burn-and-determinism.md) §3.2) | Ghi làm phương án thứ hai cho QĐ-M6. Chưa ai kiểm được Bevy ECS cùng Burn có chạy trong Wasmtime với các cờ đó không |
| Thí nghiệm nhỏ nhất: hai máy khác hệ điều hành, mỗi máy một đảo, di cư 4 con mỗi 200 tick, thêm trễ 250 ms, tắt một máy 30 giây | Hai máy đã có sẵn: Mac (`agent-mac`) và Windows (`agent-win`). Feature `networking` có lớp vận chuyển | Có thể thành một thẻ cho luồng NM, nhưng chỉ sau khi QĐ-M7 được quyết. Không mở thẻ lúc này |

### 3.3 Khuyến nghị cho QĐ-M7

Chọn hình dạng **lô cho `Contribute` và đảo cho `Inhabit`** như kế hoạch 13/11 đề xuất, và thêm chạy lại có hồi
tố vào lớp T5 khi tới phần người tình nguyện. Hai báo cáo độc lập (bản tổng hợp nghiên cứu mà kế hoạch 13/11 dựa
vào, và báo cáo này) đã tới cùng kết luận. Không lý do nào trong cả hai buộc phải quyết bây giờ: chặng 7 đứng
sau `v1.0.0`. Nếu quyết, chỉ cần một dòng trong sổ quyết định; thiết kế 25/08 được sửa trong thẻ NM-4.

## 4. Câu hỏi cho chủ dự án

| ID | Câu hỏi | Đề xuất |
|---|---|---|
| QĐ-AE-C | Thêm bốn nhánh đối chứng (trường ma, giả dược tốn phí, đẳng năng lượng, thức ăn tăng) vào ER06/ER07 của hợp đồng thí nghiệm, kèm một thẻ test trường ma trước? | Có, sau chuỗi SEL2 |
| QĐ-M7 | Lô cho `Contribute`, đảo cho `Inhabit`, cộng chạy lại có hồi tố ở T5? | Có. Quyết được bất cứ lúc nào trước NM-4 |
| — | SEL2-E báo thêm tương quan cặp đo được, làm đầu vào cho QĐ-SEL2c | Thêm vào định nghĩa SEL2-E (sửa tài liệu, không cần quyết riêng) |

## 5. Điều tài liệu này không khẳng định

- Không nói rằng đảo nhanh gấp 2,99 lần hay lockstep chậm đi 3–10 lần trên Wi-Fi. Đó là mô hình của Gemini;
  Anima chưa đo lần nào.
- Không nói rằng kiểm ngẫu nhiên 10 % bỏ sót 90 % kẻ gian. Con số đó đúng theo định nghĩa với một lần gian lận
  duy nhất và tỉ lệ kiểm 10 %, nhưng mô hình kẻ gian là của báo cáo.
- Không nói rằng RNG theo bộ đếm cần làm ngay. Nó là một khoảng trống so với ER08 và đổi mọi quỹ đạo.
- Không nói rằng Wasmtime chạy được engine này.
