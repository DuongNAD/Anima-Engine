---
title: ADR-0008 — Quá khứ bất biến, ký ức của thế giới và ba lớp thời gian (QĐ-PAST, QĐ-TIME)
status: accepted
owner: simulation-architecture
last_reviewed: 2026-11-18
decision_date: 2026-11-18
supersedes: none
superseded_by: none
---

# ADR-0008 — Quá khứ bất biến, ký ức của thế giới và ba lớp thời gian (QĐ-PAST, QĐ-TIME)

> **Trạng thái: chấp nhận ngày 2026-11-18 (thẻ L0).** Hai **nguyên tắc** đã được chủ dự án quyết trong
> chat ngày 2026-11-17:
> - **QĐ-PAST:** quá khứ bất biến, muốn quay lại thì fork, kèm bảng ngoại lệ.
> - **QĐ-TIME:** tách tick, lịch và tốc độ xem.
>
> Ngày 2026-11-18 chủ dự án trả lời năm câu hỏi còn mở của thiết kế lưu trữ và thêm một quy tắc cho việc nâng
> phiên bản engine ("Duyệt theo đề xuất"); các câu trả lời nằm ở [§4](#4-năm-câu-trả-lời-của-chủ-dự-án-2026-11-18)
> và [§5](#5-nâng-phiên-bản-engine-làm-đổi-quỹ-đạo-mở-một-kỷ-engine-mới). Đầu vào cho các câu trả lời là một báo
> cáo nghiên cứu về lưu lịch sử bất biến, đối chiếu với thiết kế dưới đây. **Chưa dòng mã nào được viết theo ADR
> này**, và gate ngân sách (H3) vẫn chưa đo: chủ dự án chấp nhận thiết kế trước khi có số.
>
> Số đo trong mục "Ngân sách" là **một lần đo thăm dò** ngày 2026-11-17 bằng test scratch (không nằm
> trong cây mã), trên thân cây `b94eb47`, macOS arm64, seed 1. Đó không phải phép đo ngân sách mà
> gate H3 bên dưới đòi hỏi.

## Bối cảnh

### Ba lớp "ký ức" của một thế giới, và ADR này nói về lớp nào

| Lớp | Là gì | Hiện trạng | Thuộc ADR này? |
|---|---|---|---|
| **Lịch sử của thế giới** | Điều gì đã xảy ra, khi nào, vì sao | Rời rạc, xem bảng dưới | **Có** |
| Ký ức của sinh vật | Một cá thể nhớ điều nó đã thấy | Não ([ADR-0003](ADR-0003-evolved-per-agent-brains.md)) là mạng truyền thẳng, không có trạng thái ẩn giữa các tick; chỉ trọng số học trong đời là thay đổi | Không. Làm sau E2R, xem "Ngoài phạm vi" |
| Ký ức của môi trường | Thế giới giữ dấu vết của hành động | Lưới pheromone (`ai/pheromone.rs`, bay hơi 0,05 mỗi tick), đồng cỏ bị gặm cạn (`ResourceField`), xói mòn và nước (`DynamicFields`), detritus | Không. Đã có và đủ dùng |

### Lịch sử hôm nay: các mảnh đã có

Các mảnh dưới đây đã được đối chiếu trên thân cây `b94eb47`:

| Mảnh | Nhớ gì | Hạn chế |
|---|---|---|
| `ChronicleEvent` (`core/simulation_state.rs`) | Sự kiện môi trường lớn (hạn hán, sóng nhiệt, sóng thú săn, dồi dào) | Một `Vec` không giới hạn trong RAM của app. Đóng dấu bằng mili-giây đồng hồ thật, trừ khi bật `DeterministicMode`. Không ghi sinh, chết hay săn mồi |
| Phả hệ (`evolution/lineage.rs`, `mrca.rs`) | Ai sinh ra ai, có MRCA | `compact` bỏ nhánh tuyệt chủng và genotype của node bị cắt (một ngoại lệ đã khai, STATE §3.15.1). Không mang nguyên nhân cái chết |
| `CausalLedger` (`anima-domain/src/causal.rs`) | Can thiệp và hệ quả, chỉ ghi thêm | Chỉ có trên đường runner thí nghiệm |
| Snapshot, checkpoint fork, `branch_tree` | Trạng thái tại một tick; các nhánh rẽ | Không kể được điều gì xảy ra *giữa* hai lần lưu |
| `autosave.json` của app | Trạng thái mới nhất | **Ghi đè**: một thế giới chạy trong app không có quá khứ |

### Bốn ràng buộc đã đo, mọi thiết kế phải tôn trọng

- **Tất định có phạm vi.** Hứa hẹn là *cùng manifest và cùng build thì cùng quỹ đạo*
  ([`DETERMINISM_CONTRACT.md`](../reference/DETERMINISM_CONTRACT.md) §1). Thẻ M1
  ([`determinism_matrix.md`](../paper/data/determinism_matrix.md)) đo được thêm hai điều:
  - Thế giới live **lệch giữa Windows, Linux và macOS**. Địa hình sinh tại chỗ đã lệch từ tick 0; trên
    world artifact đã commit thì lệch trước tick 180, vì `sin`, `cos`, `exp`, `powf` của libm khác nhau
    1 ULP giữa các nền tảng.
  - Hai máy macOS khác nhau thì trùng khít.

  Vậy phát lại quá khứ chỉ đúng tới từng bit **trên cùng build và cùng lớp nền tảng**.
- **Thế giới live lấy địa hình từ môi trường của máy.** `core::ecs::init_world` đọc `ANIMA_WORLD_ARTIFACT`,
  rồi `$TMPDIR/anima_world_artifact.anmw` (file app ghi), rồi mới tới cache địa hình trong `$TMPDIR`. Manifest
  không ghim được thế giới (E2-F2). Một bản ghi quá khứ phải biết chính xác nó chạy trên thế giới nào.
- **Đường app mặc định không tất định.** uuid thật, đồng hồ thật và AI ngoài chỉ tắt khi bật
  `DeterministicMode`. Executor đa luồng chưa từng được chạy kiểm.
- **Hot loop không được cấp phát.** Mọi system tick đều có gate zero-alloc.

## Động lực quyết định

1. **Quá khứ bất biến phải kiểm chứng được**, không chỉ là một lời hứa. Sửa một tick đã qua phải để lại dấu.
2. **Đủ để dựng lại** trạng thái ở một tick bất kỳ trong phạm vi tất định, và **đủ để kể lại**
   ("con này chết vì gì", "dòng này bắt đầu từ đâu") ở mọi phạm vi.
3. **Nhẹ.** Lưu nguyên nhân và mốc, không lưu mọi trạng thái.
4. **Không phản hồi.** Việc ghi không được đổi quỹ đạo; bật hay tắt ghi phải cho cùng checksum.
5. **Không cấp phát trên hot loop.**
6. **Tương thích với snapshot:** cửa sổ N−2 của [`SNAPSHOT_CONTRACT.md`](../reference/SNAPSHOT_CONTRACT.md)
   §5, ER08 (`run_id`, `parent_run_id`, `fork_tick`).
7. **Trung thực về nền tảng:** một bản ghi nói rõ nó phát lại được ở đâu.

## Các phương án lưu trữ quá khứ

### A. Lưu toàn bộ trạng thái ở mọi tick

Dựng lại được tức thì và không cần tất định. Nhưng một keyframe đã đo nặng khoảng 0,4–0,6 MB (mục
"Ngân sách"); nhân với 60 tick mỗi giây là khoảng 30 MB mỗi giây thực. **Loại.**

### B. Chỉ checkpoint định kỳ

Rẻ và đã có sẵn cơ chế (snapshot envelope). Nhưng nó không giữ được *vì sao*: không có sự kiện, không có
nguyên nhân cái chết. Muốn biết chuyện xảy ra giữa hai mốc thì phải phát lại, mà phát lại chỉ đúng trên
cùng nền tảng. Không đáp ứng động lực 2 ở phần "kể lại".

### C. Nguyên nhân + mốc + sổ sự kiện, móc xích bằng hash (đề xuất)

Gồm bốn phần (chi tiết ở "Quyết định"):
- phần đầu bản ghi, ghi một lần;
- **sổ đầu vào**: thứ phải phát lại, gồm can thiệp và hành động của người quan sát;
- **sổ sự kiện thế giới**: thứ để nhớ và kể lại, gồm sinh, chết, săn mồi, di cư, biên niên;
- keyframe thưa và checksum định kỳ.

Mỗi đoạn mang hash của đoạn trước.

### D. Cơ sở dữ liệu (Neo4j, SQLite) làm nguồn sự thật

Truy vấn mạnh. Nhưng nó thêm một phụ thuộc, khó bảo đảm chỉ ghi thêm, và Neo4j đã cho thấy rủi ro "lúc có
lúc không" (fallback in-memory). Một CSDL có thể dựng **từ** bản ghi C để làm chỉ mục, nhưng không làm
nguồn sự thật.

### Giữ nguyên hiện trạng

Một thế giới trong app vẫn không có quá khứ, quá khứ của thí nghiệm vẫn chỉ nằm trong các artifact rời, và
QĐ-PAST chỉ là một câu viết.

## Quyết định

### 1. Nguyên tắc QĐ-PAST (đã quyết, ghi lại)

Quá khứ của một thế giới đã chạy là bất biến. "Quay lại quá khứ" chỉ có một nghĩa: **fork**. Fork tạo một
thế giới mới từ một mốc cũ, với `run_id` riêng và `parent_run_id` / `fork_tick` trỏ về gốc; thế giới gốc
giữ nguyên. Các ngoại lệ:

| Trường hợp | Quyết định |
|---|---|
| Đổi định dạng save khi nâng phiên bản | Cho phép: đổi cách ghi, không đổi nội dung (SNAPSHOT_CONTRACT §5) |
| Xoá hẳn một thế giới | Cho phép, cả thế giới, không xoá từng đoạn |
| Nén làm mất chi tiết (ví dụ nén phả hệ, thưa keyframe) | Cho phép, nếu các sự kiện đã ghi không đổi và phần bị mất được khai trong phần đầu bản ghi |
| Một save hỏng | Không sửa tại chỗ; fork từ mốc lành gần nhất |
| Sửa trạng thái ở một tick đã qua rồi chạy tiếp như cùng một thế giới | **Không bao giờ** |

### 2. Bản ghi thế giới (World Record): phương án C

Mỗi run có một thư mục bản ghi gồm bốn phần.

**a. `header`, ghi một lần.** Gồm:
- `run_id`, `parent_run_id`, `fork_tick` và hash của keyframe tại điểm rẽ;
- fingerprint manifest, fingerprint luật, fingerprint registry;
- `WorldIdentity` **cùng hash SHA-256 của world artifact** đã dùng, để thế giới được ghim chứ không suy ra
  từ `$TMPDIR`;
- `BuildProvenance` (target, profile, rustc) và **lớp nền tảng** (OS, kiến trúc);
- `deterministic: bool`: có chạy dưới `DeterministicMode` không. Nếu không, bản ghi **nhớ được nhưng không
  phát lại được**, và nói rõ điều đó;
- danh sách các phần đã nén hoặc đã bỏ, theo ngoại lệ ở mục 1.

**b. Sổ đầu vào (inputs), chỉ ghi thêm.** Gồm can thiệp và hành động của người quan sát, mỗi mục có `tick` và
`CauseId`. Đây là thứ duy nhất cần **phát lại** cùng keyframe để dựng lại một tick. Nó trùng ý với sổ nhân
quả hiện có; bản ghi giữ một bản sao móc xích.

**c. Sổ sự kiện thế giới (events), chỉ ghi thêm: ký ức của thế giới.** Mỗi sự kiện có `tick` (không bao giờ
dùng đồng hồ thật làm khoá) và id thực thể. Loại v1:
- `Birth { child, parents, lineage }`
- `Death { agent, cause, age_ticks }`, với `cause` là một trong `starvation`, `dehydration`, `predation`,
  `reclaim`, `epoch_replacement`
- `Kill { predator, prey, eu_assimilated }`
- `Migration { agent, direction }`
- `Chronicle { kind, params }`
- `EpochReplacement { epoch }`

Sổ sự kiện là **đầu ra**: nó không bao giờ được đọc ngược vào mô phỏng, và nó kiểm chứng được bằng phát lại.

**d. Keyframe và checksum.** Keyframe là một snapshot envelope hiện có. Mật độ theo [§4](#4-năm-câu-trả-lời-của-chủ-dự-án-2026-11-18)
câu 1: dày gần hiện tại, thưa dần theo tuổi, không thưa hơn **một keyframe mỗi năm mô phỏng** (6.000 tick).
Thêm `world_checksum` ở các mốc dày hơn (đề xuất mỗi 600 tick) để một lần phát lại tự kiểm được.

**Móc xích.** Sổ được ghi theo đoạn. Mỗi đoạn mang `tick_from`, `tick_to`, `prev_hash` và
`hash = SHA-256(prev_hash ‖ bytes chuẩn của đoạn)` (dùng `core::sha256`). Đoạn đầu móc vào hash của
`header`, và hash của keyframe nằm trong đoạn phủ tick của nó. Sửa một byte trong quá khứ sẽ làm gãy chuỗi
từ chỗ đó trở đi.

**Đường ghi.** System tick đẩy sự kiện vào một **vòng đệm cấp phát sẵn**, kích thước cố định, nên không cấp
phát. Một thread ghi riêng gom vòng đệm thành đoạn rồi ghi nguyên tử theo SNAPSHOT_CONTRACT §4.
- **Tràn đệm** không bao giờ được lặng lẽ: số sự kiện bị bỏ được đếm và ghi vào đoạn kế tiếp, và một bản ghi
  có tràn thì không được gọi là đầy đủ. Đã quyết: **bỏ và đếm**, không chặn sim ([§4](#4-năm-câu-trả-lời-của-chủ-dự-án-2026-11-18) câu 2).

**Phát lại tới tick t.** Nạp keyframe gần nhất ≤ t, chạy mô phỏng từ đó cùng sổ đầu vào, rồi so
`world_checksum` ở các mốc. Chỉ có hiệu lực khi build và lớp nền tảng trùng với `header`, và
`deterministic = true`. Ở nơi khác, bản ghi vẫn **đọc** được: sổ sự kiện và keyframe là dữ liệu, không cần
phát lại.

**Fork** tạo một bản ghi mới. `header` của nó trỏ về `(parent run_id, fork_tick, hash keyframe)`; bản ghi gốc
không bị ghi thêm gì. Đây đúng là mô hình của `branch_tree` và ER08.

**App desktop.** `autosave.json` được thay bằng bản ghi của thế giới đang chạy; một "save" mới là một mốc
trong bản ghi, không ghi đè.

### 3. QĐ-TIME: ba lớp thời gian (đã quyết, ghi lại)

| Lớp | Là gì | Nơi sống | Có đổi quỹ đạo? |
|---|---|---|---|
| **Tick và vòng đời** | 60 tick mỗi giây mô phỏng, epoch 1.000 tick, năm 6.000 tick ([`SIMULATION_RULES.md`](../../SIMULATION_RULES.md) §4) | Hằng số của luật | Có, chính là luật |
| **Lịch của thế giới** | Bao nhiêu tick là một ngày, một năm; tên mùa | **Tham số khai báo của thế giới**; vắng thì như hôm nay (năm 6.000 tick, chưa có "ngày"). Lịch làm đổi nhịp mùa thì là luật, nằm trong `WorldLawSet` và bất biến trong một run (ER01) | Chỉ khi nó là luật |
| **Tốc độ xem** | Bao nhiêu tick mỗi giây thực; "một ngày bằng 2 giờ thực" là một mức đặt sẵn | **Chỉ ở UI**, không vào manifest hay bản ghi | **Không bao giờ** |

Hệ quả cho bản ghi: mọi khoá là `tick`. Lịch chỉ dùng để **hiển thị** tuổi thế giới và tuổi sinh vật. Đưa
chu kỳ ngày–đêm vào luật (ánh sáng, nhiệt) là một thay đổi luật riêng: nó cần sửa `SIMULATION_RULES.md` và
test trong cùng thay đổi, và không thuộc ADR này. Chu kỳ ngày–đêm của cảnh 3D (`sceneClock.ts`) vẫn là đồng
hồ hiển thị.

### 4. Năm câu trả lời của chủ dự án (2026-11-18)

| # | Câu hỏi | Quyết định |
|---|---|---|
| 1 | Mật độ keyframe | **Dày gần hiện tại, thưa dần theo tuổi, có sàn.** Keyframe mới được ghi dày; khi chúng cũ đi, các keyframe không được ghim bị thưa dần (khoảng cách gấp đôi theo mỗi bậc tuổi) nhưng không bao giờ thưa hơn một keyframe mỗi năm mô phỏng (6.000 tick). Keyframe ở điểm rẽ của mọi fork và ở mốc người dùng đánh dấu được **ghim vĩnh viễn**. Bỏ một keyframe không ghim là ngoại lệ "thưa keyframe" ở mục 1 và được khai trong `header` |
| 2 | Vòng đệm sự kiện tràn | **Bỏ và đếm.** Mô phỏng không bao giờ chậm vì ghi; số sự kiện bị bỏ được ghi vào đoạn kế tiếp, và một bản ghi có tràn bị đánh dấu "không đầy đủ" |
| 3 | App tự bật `DeterministicMode` khi người dùng muốn "có quá khứ" | **Có.** Không tất định thì bản ghi chỉ nhớ được chứ không phát lại được |
| 4 | Lưu đồng hồ thật | **Có, chỉ làm trường phụ** để hiển thị cho người xem; không bao giờ làm khoá, và không bao giờ đọc ngược vào mô phỏng |
| 5 | Trần dung lượng | **Không chặn cứng.** Chính sách thưa ở câu 1 giữ dung lượng thấp. Khi một thế giới vượt ngưỡng cảnh báo (mặc định 5 GB, chỉnh được), app báo và đề nghị nén; nó không tự xoá và không dừng mô phỏng |

Vì sao bỏ một keyframe cũ không phá QĐ-PAST: keyframe là bản đệm dựng lại được từ keyframe trước, sổ đầu vào và
mô phỏng tất định. Sổ đầu vào, sổ sự kiện và chuỗi hash thì không bao giờ bị thưa.

### 5. Nâng phiên bản engine làm đổi quỹ đạo: mở một kỷ engine mới

Một bản ghi chỉ phát lại được bằng đúng engine đã ghi nó. Khi một thay đổi mã làm đổi quỹ đạo, ví dụ một lần
tăng `LIVE_MODEL_VERSION` hay thẻ R1c (nâng `burn`, schema snapshot 8), sổ đầu vào cũ **không bao giờ** được
phát lại bằng engine mới.

- Một bản ghi là một chuỗi **kỷ engine** (engine era). Đây không phải "epoch" 1.000 tick của tiến hoá.
- Mỗi kỷ mở bằng một keyframe đầy đủ và một `header` riêng ghi phiên bản engine (`LIVE_MODEL_VERSION`,
  `BuildProvenance`, `SCHEMA_VERSION`). Header của kỷ móc vào hash của đoạn cuối kỷ trước.
- Chuyển kỷ: lấy trạng thái cuối do engine cũ ghi (keyframe cuối kỷ trước), nạp bằng engine mới theo đường
  chuyển đổi đã khai của SNAPSHOT_CONTRACT, rồi ghi keyframe đầu kỷ mới. Mọi mất mát của lần chuyển (ví dụ
  learner dùng chung khởi tạo lại theo QĐ-R1c-a) được khai trong header của kỷ mới.
- Kỷ cũ vẫn **đọc** được (sổ sự kiện và keyframe là dữ liệu), nhưng chỉ **phát lại** được bằng engine của kỷ
  đó. Giữ lại bản build cũ để phát lại là tuỳ chọn, không bắt buộc.

## Ngân sách (thăm dò, một lần đo)

| Đại lượng | Đo được | Nguồn |
|---|---|---|
| Một keyframe, thế giới mặc định, 10 agent, tick 6.000 | **421.395 byte** JSON chưa nén | test scratch, `serde_json::to_vec(&LiveSnapshot.saved)` |
| Một keyframe, cấu hình P1, 83 agent, tick 6.000 | **589.799 byte**, trong đó phần agent 158.846 byte (khoảng 1,9 KB mỗi agent) | như trên |
| Sự kiện sinh và chết, cấu hình P1, 6.000 tick | 128 + 55, tức khoảng **0,03 sự kiện mỗi tick** | như trên |
| World artifact đã commit | 1.114.148 byte, ghi một lần, chia sẻ giữa các run | `artifacts/world_256.anmw` |

Suy ra, **không phải đo**: ở 60 tick mỗi giây và một keyframe mỗi năm mô phỏng, một giờ thực tương ứng 36
năm mô phỏng, tức khoảng 21 MB keyframe JSON chưa nén. Sổ sự kiện ở mật độ trên chỉ khoảng 6.500 sự kiện
mỗi giờ. **Keyframe chiếm gần hết dung lượng**, nên mật độ keyframe và nén là hai núm chỉnh chính. Gate H3
phải đo lại trên phần cứng đích, với quần thể lớn và có nén.

## Hệ quả

### Tích cực

- QĐ-PAST thành thứ kiểm được: một byte quá khứ bị sửa sẽ làm gãy chuỗi hash.
- **Thế giới đọc được** có nguồn dữ liệu thật để kể chuyện (nguyên nhân chết, dòng dõi, sự kiện), và thẻ S5
  (line of descent) có sự kiện theo tick để bám.
- Thế giới được **ghim**, nên hidden input `$TMPDIR` và E2-F2 được giải quyết cho mọi run có bản ghi.
- Fork trở thành thao tác bình thường cho người dùng ("quay lại năm 12 và thử khác đi"), không phá gì.

### Tiêu cực / rủi ro

- Thêm một thread ghi và I/O đĩa; vòng đệm phải đủ lớn cho đỉnh sự kiện (một epoch replacement thay hàng
  loạt cá thể cùng lúc).
- Phát lại bị giới hạn theo nền tảng (M1). Một bản ghi chia sẻ giữa người dùng khác hệ điều hành chỉ **đọc**
  được, không phát lại được, cho tới khi có QĐ-M6 hoặc một libm tất định.
- Đường app phải bật `DeterministicMode` thì bản ghi mới phát lại được, nghĩa là executor đơn luồng và mất
  song song. Đó là giá phải trả cho mỗi thế giới muốn có quá khứ dựng lại được.
- Dung lượng tăng tuyến tính theo thời gian chạy, nên cần một chính sách giữ hoặc nén được khai.

## Ngoài phạm vi

- **Ký ức của sinh vật.** Làm sau E2R, khi đã đo được não có tạo khác biệt. Nếu làm, phải qua một ADR riêng
  với các ràng buộc:
  - dung lượng nhớ nằm trong gen và tốn năng lượng qua `total_cost`;
  - nội dung nhớ chết theo cá thể (không Lamarck, ADR-0003);
  - save, restore và migration mang theo nó;
  - bộ đệm cố định kích thước;
  - có một bản đăng ký trước.
- Chu kỳ ngày–đêm trong luật (xem QĐ-TIME).
- Đồng bộ bản ghi giữa nhiều máy (chặng 7, QĐ-M3…M7).

## Kế hoạch triển khai và hoàn tác

Mỗi bước là một thẻ riêng. Bước sau dựa vào bước trước, và mỗi bước đều tắt được: vắng bản ghi thì mọi thứ
chạy như hôm nay.

1. **H1 — Định dạng và không phản hồi.** Kiểu `header`, đoạn, sự kiện; mã hoá chuẩn; móc xích. Gate: bật hay
   tắt ghi cho cùng checksum ở 0/180/1.800/18.000 tick (so lượt với lượt, không ghim tuyệt đối); một byte bị
   sửa làm gãy chuỗi ở đúng đoạn đó.
2. **H2 — Đường ghi.** Vòng đệm cấp phát sẵn và thread ghi. Gate: zero-alloc trên tick có sự kiện; tràn đệm
   được đếm và ghi lại; ghi nguyên tử, tức process bị giết giữa chừng không làm hỏng đoạn đã xong.
3. **H3 — Keyframe, phát lại và fork.** Gate: dựng lại tick t bằng keyframe và sổ đầu vào cho đúng
   `world_checksum` ghi ở mốc, trên cùng nền tảng; một fork không ghi gì vào bản ghi gốc; đo ngân sách trên
   phần cứng đích.
4. **H4 — App và đọc.** Thay `autosave.json`, thêm lệnh IPC để đọc sự kiện theo cá thể, dòng dõi và tick
   (vào [`IPC_CONTRACT.md`](../reference/IPC_CONTRACT.md) cùng commit), UI dòng thời gian.
5. **H5 — Chính sách giữ và nén.** Thưa keyframe theo §4 câu 1, ngưỡng cảnh báo theo §4 câu 5, và nén
   keyframe (đổi từ JSON sang nhị phân có nén là bước rẻ nhất); phần bị bỏ được ghi trong `header`.

Hoàn tác: bản ghi là thư mục riêng cạnh save. Gỡ tính năng thì không động tới snapshot hay trạng thái nào; các
save cũ vẫn đọc được theo SNAPSHOT_CONTRACT.

## Câu hỏi cho chủ dự án trước khi `accepted`

Đã trả lời hết ngày 2026-11-18; xem [§4](#4-năm-câu-trả-lời-của-chủ-dự-án-2026-11-18).

## Bằng chứng xác minh

| Gate | Lệnh / artifact | Ngưỡng | Kết quả |
|---|---|---|---|
| Không phản hồi | So checksum ghi bật và ghi tắt, cùng seed | Trùng ở mọi mốc | pending (H1) |
| Bất biến | Sửa một byte trong đoạn k | Chuỗi gãy đúng từ k | pending (H1) |
| Zero-alloc | Bộ đếm cấp phát trên tick có sự kiện | 0 | pending (H2) |
| Phát lại | Keyframe + sổ đầu vào → tick t | `world_checksum` trùng mốc, cùng nền tảng | pending (H3) |
| Ngân sách | Đo trên phần cứng đích, quần thể lớn, có nén | Ghi vào ADR khi H3 xong (chủ dự án chấp nhận thiết kế trước khi có số, 2026-11-18) | pending (H3) |

## Tài liệu bị ảnh hưởng

- Contract/reference:
  - [`SNAPSHOT_CONTRACT.md`](../reference/SNAPSHOT_CONTRACT.md): keyframe là snapshot envelope.
  - [`DETERMINISM_CONTRACT.md`](../reference/DETERMINISM_CONTRACT.md): phạm vi phát lại theo nền tảng.
  - [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md): ER08, ghim thế giới.
  - [`IPC_CONTRACT.md`](../reference/IPC_CONTRACT.md): lệnh đọc mới, ở H4.
  - [`SIMULATION_RULES.md`](../../SIMULATION_RULES.md) §4: lịch là tham số khai báo.
- Planning: thẻ L0 (ADR này, xong khi `accepted`), năm thẻ WR1–WR5 cho các bước H1–H5 (kế hoạch swarm), S5 (dùng sổ sự kiện), R1c (lần chuyển kỷ engine đầu tiên).
- Nguồn của các nguyên tắc: bản ghi QĐ-PAST và QĐ-TIME của roadmap 31/10 §4.2 (PR #97).
