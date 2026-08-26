---
phase: testing
feature: living-world
title: Testing — Thế giới sống
description: Gate cho hình thái, prompt, animation và tính toán phân tán
status: proposed
owner: simulation-architecture
last_reviewed: 2026-08-25
requirements: ../requirements/2026-08-25-feature-living-world.md
planning: ../planning/2026-08-25-feature-living-world.md
contract: ../../reference/CREATURE_PROMPT_CONTRACT.md
---

# Testing — Thế giới sống

Quy tắc chung của repo áp dụng nguyên: một gate xanh phải là **kết quả một lệnh chạy được**, có
ngày, có cấu hình. Số không kèm lệnh là lịch sử, không phải trạng thái —
[STATE_OF_THE_PROJECT §1.1](../../planning/STATE_OF_THE_PROJECT.md).

---

## 1. Gate hình thái (GM)

| Gate | Bằng chứng | Bắt được gì |
|---|---|---|
| **GM-S01** | Cùng `DevelopedPhenotype` ⇒ cùng `MorphoSpec`, byte-exact | trạng thái ẩn lọt vào IR |
| **GM-S02** | Cùng `MorphoSpec` + seed ⇒ mesh byte-exact (vertex order, index, weight) | vòng lặp phụ thuộc thứ tự HashMap |
| **GM-S03** | Kích thước mesh khớp `Segment`/`SpatialCollider` trong tolerance công bố | hình vẽ và hình va chạm tách nhau |
| **GM-S04** | Đổi một gene lượng nhỏ ⇒ mesh đổi **liên tục**, không nhảy bậc | band hoá quá thô ở chỗ quan trọng |
| **GM-S05** | Compile mesh 1.000 agent trong ngân sách frame; hot path `allocs == 0` | vi phạm quy tắc zero-alloc |
| **GM-S06** | Gỡ toàn bộ thư mục asset ⇒ app chạy y hệt, chỉ mất trang trí | asset bò vào đường sống |
| **GM-S07** | `ANIMA_DETERMINISTIC=1` ⇒ **không** request mạng nào từ pipeline hình thái | rò rỉ thứ sáu |
| **GM-S08** | Checksum run không đổi khi asset xuất hiện/biến mất | asset lọt vào hash canonical |

GM-S03 là gate quan trọng nhất của nhánh A: nó là điều duy nhất giữ cho "con vật nhìn thấy" và
"con vật va chạm" là **một** cơ thể.

---

## 2. Gate prompt (PR)

PR-S01…PR-S10 định nghĩa tại [`CREATURE_PROMPT_CONTRACT §10`](../../reference/CREATURE_PROMPT_CONTRACT.md).
Ghi chú thi hành:

- **PR-S02** chạy trên **fixture quần thể**, không trên vài ví dụ viết tay. Nguồn fixture: một run
  headless đã lưu, lấy toàn bộ phenotype, sinh prompt cho tất cả, đo phân bố độ dài.
- **PR-S04** (không band chết) cần một **báo cáo phủ**: mỗi nhánh lexicon kèm số fixture chạm tới.
  Nhánh có 0 là fail, không phải cảnh báo. Đây là gate rẻ nhất và bị bỏ nhiều nhất.
- **PR-S07** thi hành bằng danh sách cấm: nếu nhà cung cấp có tham số `symmetry`, thì chuỗi
  `symmetr` không được xuất hiện trong `prompt.text`.

---

## 3. Gate animation (AN)

| Gate | Bằng chứng | Bắt được gì |
|---|---|---|
| **AN-S01** | Đổi gait không reset `phase`; pha liên tục qua mọi chuyển trạng thái | giật chân ở mỗi lần chuyển |
| **AN-S02** | Blend theo **thời gian**, không theo tick: kết quả giống nhau ở 30/60/144 Hz | animation phụ thuộc tần số |
| **AN-S03** | Gait hợp lệ cho fixture 0/2/4/6/8 chi | gait hardcode cho bốn chân |
| **AN-S04** | `energy ≤ 0` ⇒ `CpgOscillator.output == 0` (hành vi hiện có, không được phá) | regression trên `cpg.rs` |
| **AN-S05** | Cue dẫn xuất khớp trạng thái mô phỏng trên toàn bộ một run ghi lại | animation có state riêng, trôi |
| **AN-S06** | Ưu tiên overlay: `Die > Hurt > Attack > Feed > Drink`, không chồng lấn sai | hai overlay cùng chạy |
| **AN-S07** | Vị trí vẽ == `Position` ± tolerance nội suy | root motion lọt vào animation |
| **AN-S08** | `AgentCue` ≤ 20 byte/agent; giãn cách K tick vẫn mượt | payload phình theo quần thể |

AN-S07 là gate bảo vệ ranh giới: animation được phép *diễn giải*, không được phép *di chuyển*.

---

## 4. Gate liên bang (FD)

| Gate | Bằng chứng | Bắt được gì |
|---|---|---|
| **FD-S01** | `HibernateAgentCommand`: chuyển ngăn + despawn tại **một** sync point; residual ≤ `1e-3` | cửa sổ hai lần ghi có, đúng lỗi §3 ledger contract |
| **FD-S02** | `WakeAgentCommand` **không** gọi development; phenotype và brain giữ nguyên (D01/D02) | cá thể "sinh lại" thành con khác |
| **FD-S03** | hibernate → save → load → wake: residual ≤ `1e-3`, phenotype bit-exact | save làm mất agent dormant |
| **FD-S04** | **Rút mạng đột ngột** giữa run: residual ≤ `1e-3` | mất EU theo thiết bị — rủi ro số một |
| **FD-S05** | `contribute_matches_solo` — chế độ chỉ-góp-CPU cho quỹ đạo **bằng** một máy | phân công lọt vào kết quả |
| **FD-S06** | Gộp theo `shard_id`, không theo thứ tự đến: hoán vị thứ tự gói ⇒ cùng checksum | phụ thuộc thứ tự mạng |
| **FD-S07** | T2 từ chối batch tạo EU; T3 từ chối teleport; T4 từ chối ghi agent không được giao | shard hỏng/gian lận |
| **FD-S08** | Kiểm chéo T5: batch bị sửa cố ý ⇒ shard bị cách ly, agent khôi phục từ bản bóng | kiểm chéo không thực sự chạy |
| **FD-S09** | `agent_budget` ≥ 1 với thiết bị yếu nhất trong fixture | máy yếu nhận 0 sinh vật |
| **FD-S10** | Ngân sách có hysteresis: `sustained_tps` dao động ±10% ⇒ ngân sách không đổi | quần thể phập phồng theo nhiệt CPU |

### 4.1 FD-S04 là gate không được bỏ

Cách chạy: hai process, phân dải, chạy N tick, **kill -9** một process, để host phát hiện timeout,
đo residual. Không dùng ngắt lịch sự — ngắt lịch sự là đường dễ, và đường dễ không phải đường sẽ
xảy ra khi ai đó gập laptop.

---

## 5. Ma trận gate → yêu cầu

| Yêu cầu | Gate |
|---|---|
| R1.1 kích thước thật | A1 gate hiện có, GM-S03 |
| R1.2 ngoại hình theo gene/môi trường | PR-S01, PR-S04 |
| R1.3 prompt chuẩn ≤ 1000 ký tự | PR-S02, PR-S03, PR-S07 |
| R1.4 không đổi theo biome | GM-S01 (IR không đọc môi trường hiện tại) + Gate P5 morphogenesis |
| R1.5 xuất hiện ngay | GM-S06, PR-S08 |
| R2.1–R2.3 cử động, mượt, đúng hình thái | AN-S01…AN-S04 |
| R2.4 không có state riêng | AN-S05, AN-S07 |
| R3.1–R3.3 phân tán, ngân sách theo phần cứng | FD-S09, FD-S10 |
| R3.4 xuất hiện/biến mất theo thiết bị | FD-S01, FD-S02 |
| R3.5 không mất EU | **FD-S03, FD-S04** |
| R3.6 góp CPU == một máy | **FD-S05**, FD-S06 |
| N1 residual | FD-S01…FD-S04 |
| N3 trần chi phí API | PR-S09 |
| N7 gỡ được tính năng mới | GM-S06, GM-S08, PR-S08 |

---

## 6. Cách chạy

Lệnh backend giữ nguyên quy ước repo — **luôn kèm `--features desktop`**, và bắt output bằng
`cmd /c "… > out.txt 2>&1"` chứ không bằng pipeline PowerShell
([CLAUDE.md](../../../CLAUDE.md)):

```bash
cargo test --features desktop --no-fail-fast
```

Gate liên bang cần feature `networking`, nên chúng phải có `#![cfg(feature = "networking")]`
**và** một mục `[[test]] required-features` khớp — nếu không, `scripts/check_test_targets.mjs` sẽ
bắt, đúng như nó đã bắt 1.877 dòng coverage bị bỏ qua âm thầm trước đây.

```bash
node scripts/check_test_targets.mjs out.txt --profile desktop
```

---

## 7. Điều bộ gate này **không** chứng minh

- Không chứng minh model 3D **đẹp**. Không gate tự động nào làm được; đó là việc của A8 (duyệt
  bằng mắt, một lần cho mỗi archetype).
- Không chứng minh animation **trông tự nhiên**. Nó chứng minh animation *đúng* và *liên tục*.
- Không chứng minh hệ chịu được đối thủ có động cơ. FD-S07/S08 chặn shard hỏng và gian lận nghiệp
  dư.
- Không chứng minh app chạy được dưới executor đa luồng của desktop app đầy đủ — đó vẫn là khoảng
  trống đã biết của repo, và các gate ở đây chạy headless như mọi gate khác.
