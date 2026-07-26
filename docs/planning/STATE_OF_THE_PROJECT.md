---
title: Trạng thái dự án và việc cần làm tiếp
status: active
owner: maintainers
last_reviewed: 2026-07-26
review_cycle: per-release
---

# Trạng thái dự án và việc cần làm tiếp

Tài liệu **sống**. Đây là chỗ một phiên mới đọc đầu tiên để biết dự án đang ở đâu và
làm gì kế tiếp. Nó không định nghĩa lại hợp đồng nào — hợp đồng ở
[`docs/reference/`](../reference/README.md) và các ADR. Nó chỉ trả lời hai câu:
**điều gì đã đo được là đúng**, và **việc gì đáng làm tiếp, theo thứ tự nào**.

Quy tắc cập nhật: khi một mục ở §3 xong, chuyển nó lên §2 kèm số đo, đổi
`last_reviewed`, và ghi một dòng vào [`TODO.md`](../../TODO.md). Đừng đánh dấu xong khi
gate chưa xanh.

---

## 1. Trạng thái đo được (2026-07-26)

Toàn bộ số dưới đây được chạy lại trong ngày, trên `main` tại `c0a3cff`, cây làm việc sạch.
Đây là số đo, không phải trích dẫn tài liệu.

| Gate | Lệnh | Kết quả |
|---|---|---|
| Backend test | `cargo test --features desktop --no-fail-fast` | **629 pass · 0 fail · 4 ignored**, 67 test binary, 0 warning biên dịch |
| Target rỗng | `node scripts/check_test_targets.mjs <output>` | 65 target, **0 target chạy rỗng** |
| Format | `cargo fmt --check` | sạch |
| Lint backend | `cargo clippy --all-targets --features desktop -- -D warnings` | sạch |
| Test frontend (src) | `npm run test` | 13 file · **90 pass** |
| Test frontend (tests/) | `npm run test:frontend` | 26 file · **243 pass**, 1 skip |
| Lint frontend | `npm run lint` + `node scripts/eslint_ratchet.mjs` | **0 error**, 491 warning (baseline 491) |
| Build | `npm run build` | pass |
| Link tài liệu | `node scripts/check_docs_links.mjs` | 406 link trong 90 file, **0 gãy** — đo lại 2026-07-26 |

Quy mô: Rust ~47,7k dòng / 128 file · TS ~25,6k dòng / 126 file · 627 hàm `#[test]` ·
62 file test tích hợp backend · 46 file test frontend · 7 spec Playwright · 45 tài liệu.

> **Chỉ hàng "Link tài liệu" được đo lại ngày 2026-07-26** (đợt review nguồn mở + chuẩn hoá tài
> liệu, xem [`TODO.md`](../../TODO.md)). Các hàng còn lại và dòng quy mô ở trên vẫn là số của lần
> chạy tại `c0a3cff` và **chưa** được chạy lại trong phiên đó — đợt review không đụng tới code.

Chỉ số kỷ luật đáng giữ, vì mất đi thì khó lấy lại:

- **5** `.unwrap()/.expect()` trong toàn bộ Rust *production* (phần còn lại nằm trong `#[cfg(test)]`).
- **2** khối `unsafe` trong cả backend — cả hai ở [`ai/model.rs:360`](../../src-tauri/src/ai/model.rs).
- **3** marker `TODO/FIXME` trong mã nguồn.

---

## 2. Điều đã đúng, và đúng ở mức nào

Dùng thang bậc của chương trình G0–G4 (xem
[`docs/ai/planning/2026-07-25-claude-overnight-goal-g0-g4-remediation.md`](../ai/planning/2026-07-25-claude-overnight-goal-g0-g4-remediation.md)),
vì "DONE" một mình không phân biệt được "có hàm thuần đã test" với "đường sống chạy qua nó".

| Vùng | Bậc đạt được | Ghi chú |
|---|---|---|
| Hạ tầng kiểm thử và gate CI | Live integrated | Gate nhắm vào chế độ hỏng thật, không phải tick checkbox — xem §2.1 |
| Năng lượng đóng (EU) ở thế giới sống | Live integrated | G1.1 — bit-exact, không phải "trong dung sai" |
| Snapshot/checkpoint | Live integrated | G1.2 — `rand_chacha` trực tiếp để lưu được vị trí draw |
| Lõi tất định | Live integrated | G1.3 — `SimRng`, `BTreeMap` thay `HashMap`, test quét mã nguồn chặn `thread_rng()` |
| Hợp đồng Rust↔TS | Live integrated | G1.4 — gate parity `ts-rs` trong CI |
| Tách feature build | Live integrated | G2 gate #2 — CI kiểm bằng `cargo tree`, không chỉ "biên dịch được" |
| Trần tài nguyên runner | Live integrated | G2 gate #3 — `MAX_ENSEMBLE_RESULT_BYTES`, ước lượng bão hoà thay vì tràn |
| Não tiến hoá per-agent (ADR-0003) | Đã triển khai, **tắt mặc định** | 11/12 gate EB pass — xem §3.1 |
| Lab tiến hoá AE1–AE3 | Headless, opt-in | `ReferenceEvolutionWorld`; thế giới Bevy sống **chưa** experiment-ready |
| Thế giới chung frontend↔backend | Live integrated | `src/utils/sharedWorld.ts` là identity duy nhất; artifact đẩy sang `init_world` |

### 2.1 Vì sao bộ gate này đáng tin

Ghi lại để phiên sau không vô tình gỡ mất:

- `scripts/check_test_targets.mjs` bắt **target biên dịch thành binary rỗng rồi exit 0**.
  Đây là chế độ hỏng đã giấu 1.877 dòng coverage: bảy file có `#![cfg(feature = ...)]`
  ở cấp crate, và một `cargo test` trần biến chúng thành `running 0 tests`.
- CI kiểm tách feature bằng **đồ thị phụ thuộc** (`cargo tree`), không phải bằng việc
  biên dịch được. Biên dịch được sẽ vẫn xanh vào lần đầu ai đó thêm một `use` vô điều kiện.
- Gate parity `ts-rs`: `cargo test` sinh `src/types/generated/`, rồi `git diff --exit-code`.
  Đây là thứ bắt được `head_directions` là map ở một phía và array ở phía kia.
- `sim_determinism_tests.rs` có một test **quét mã nguồn** và fail nếu `thread_rng()` quay lại.
- Ratchet ESLint chặn warning tăng, kể cả khi không ai giảm.

---

## 3. Việc cần làm, theo thứ tự

Thứ tự là **theo giá trị trả về**, không theo độ khó. Mỗi mục có điểm neo cụ thể và một
định nghĩa hoàn thành kiểm tra được.

### P0 — Đóng vòng lặp khoa học

Đây là khoảng cách thật của dự án. Chất lượng kỹ thuật đã cao; cái thiếu là **bằng chứng
trên đường mặc định**.

#### 3.1 Bật não tiến hoá per-agent trên đường mặc định

**Vì sao P0.** Một run mặc định hiện cho **mọi agent dùng chung một `BrainModel`**. Đó
chính là gap mà [`docs/research/MAP_AND_ML_UPGRADE_RESEARCH.md`](../research/MAP_AND_ML_UPGRADE_RESEARCH.md)
gọi là lớn nhất. Toàn bộ máy móc đã có và đã test — nó chỉ đang tắt.

**Trạng thái thật:** 11/12 gate EB pass (bảng gate trong
[ADR-0003](../decisions/ADR-0003-evolved-per-agent-brains.md)). Chỉ **EB-S04** là 🟡 một phần.

**Điểm neo:** `BrainPolicy::default()` tại
[`src-tauri/src/core/resources.rs`](../../src-tauri/src/core/resources.rs) — `evolved: false`,
`lifetime_learning` tắt, `brain_metabolic_cost: 0.0`. Cờ `ANIMA_EVOLVED_BRAINS`,
`ANIMA_LIFETIME_LEARNING`.

**Việc thật cần làm trước tiên là quyết định về EB-S04, không phải lật cờ.** EB-S04 đòi
quỹ đạo *bit-identical* với baseline. Nó fail vì khởi tạo model dùng chung đã đổi từ
ngẫu nhiên sang **có seed** — tức là fail vì một **cải tiến có chủ ý**, không phải vì hồi quy.
Một gate không thể pass bằng cách sửa code đúng thì phải được **re-baseline một cách tường minh**:
đặt lại mốc so sánh về bản dựng có seed, ghi vào ADR *vì sao* mốc cũ bị bỏ, rồi mới bàn đến mặc định.

**Định nghĩa hoàn thành:** EB-S04 chuyển 🟡 → ✅ với mốc mới ghi rõ trong ADR-0003 · quyết định
mặc định (`evolved: true` hay giữ opt-in) được ghi thành mục quyết định trong ADR · nếu bật mặc định
thì `cargo test --features desktop` vẫn xanh và EB-S03 (`allocs == 0`) vẫn giữ.

#### 3.2 Thay số hiệu năng proxy bằng số đo thật — **một nửa đã xong (2026-07-26)**

**Vì sao vẫn P0.** Tuyên bố "60 FPS real-time" của dự án nay **đã có số đỡ, nhưng chưa được chứng
minh**. Mọi quyết định về scale (LOD, ngân sách bộ nhớ não, số agent thường trú) trước đây dựa trên
ước lượng; giờ chúng dựa trên một **cận dưới đo được**, không phải một khung hình đo được.

**Ràng buộc vận hành có thật:** không chạy `npm run tauri dev` / `cargo run` trên máy dev.
Đây là ràng buộc, không phải lời khuyên — nó đã crash máy.

##### Đã xong

- **OSS-010 Criterion đã ship.** `dev-dependency` + [`src-tauri/benches/tick_systems.rs`](../../src-tauri/benches/tick_systems.rs),
  bench từng system headless — không Tauri, không GPU device. Bảng số và cảnh báo diễn giải:
  [`docs/how-to/BENCHMARKING.md`](../how-to/BENCHMARKING.md).
- **Phần cứng mục tiêu đã đổi và đã khớp.** Nay là Intel Core i5-14600KF; khai báo *Dell Vostro
  3530* cũ **không còn hiệu lực**. Máy capture và máy đích là một, nên số đo tính là đo-trên-đích.
- **`benchmark_report.json` mang 16 số đo thật** dưới tiền tố `criterion/`, kèm khối hardware đúng.
  `scripts/bench_baseline.mjs` đọc thẳng từ output Criterion, và **từ chối** ghi đè số thật bằng
  proxy (exit ≠ 0) trừ khi `ANIMA_BENCH_ALLOW_PROXY_ONLY=1`.
- Gate `cargo tree --no-default-features -e normal` xác nhận Criterion **không** vào bản dựng mặc
  định (G2 #2), và test S04 vẫn xanh.

##### Còn thiếu — và không còn vì phần cứng

Cái đo được là **cận dưới của một tick, không phải khung hình**: tổng các system chạy mỗi tick ở
1.000 agent ≈ 493 µs ≈ 3,0 % của 16,67 ms, nhưng con số đó **chưa gồm** suy luận não, lập lịch ECS,
change detection, thread emit, va chạm và trao đổi chất.

1. **In-app tick capture** cho các hàng `Physics tick` / `Brain/sensor` / `Full-brain agents` trong
   [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md). Ràng buộc "không chạy full backend" vẫn
   còn, nên đây cần một harness đo tick an toàn — chưa có.
2. **Suy luận não per-agent** là phần đắt nhất còn lại và **đang tắt mặc định** (§3.1), nên chưa có
   gì trên đường mặc định để đo. §3.1 và §3.2 vì thế nối vào nhau.

**Định nghĩa hoàn thành (phần còn lại):** ba hàng nói trên có số thật · `BENCHMARK_BASELINE.md`
tuyên bố khoá số theo `WORLD_SIMULATION_PLAN.md` §10.2 · ngân sách EB-S12 (22,5 KiB/agent,
~46.500 agent/GiB) được đối chiếu với số thật thay vì với ngoại suy tuyến tính hiện tại (~4,02 ms
≈ 24 % khung hình, **là ngoại suy chứ không phải phép đo**).

**Một finding mở, sinh ra từ chính đợt đo:** doc comment của `ResourceField::REGROWTH_STRIDE` ghi
đường trước khi stride tốn ~4,2 ms/tick; đo lại release build cho **~0,36 ms**, thấp hơn ~12 lần.
Việc stride vẫn đúng và có lợi (đo được 3,97×) — chỉ con số biện minh cho nó là chưa đối chứng được.
Theo quy tắc 6 của [chính sách tài liệu](../governance/DOCUMENTATION_POLICY.md), đây là finding cần
đối chứng, không phải lỗi đã xác định.

#### 3.3 Đưa thế giới Bevy sống qua gate thí nghiệm

**Vì sao P0.** Phần khoa học (AE1–AE3) hiện nằm ở `ReferenceEvolutionWorld` **headless**.
CLAUDE.md cấm tuyên bố thế giới sống là experiment-ready cho tới khi adapter tất định và
gate persistence của nó pass — và lệnh cấm đó vẫn đang đúng. Chừng nào chưa qua, "mô phỏng
tiến hoá" và "thí nghiệm tiến hoá" là hai hệ thống khác nhau.

**Điểm neo:** `src-tauri/src/core/reference_world.rs`, `core/evolution_pathway.rs`,
`core/scenario.rs`. Đọc [`docs/reference/EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md) trước.

**Điều kiện tiên quyết — đọc trước khi bắt tay:** mục này **và** §3.6 là hai mặt của cùng một việc,
chính là **G2 task 1** trong
[kế hoạch G0–G4](../ai/planning/2026-07-25-claude-overnight-goal-g0-g4-remediation.md) ("Headless và
Bevy trở thành hai adapter trên `anima-domain`… AE4 *là* sự hội tụ này"). Không thể đưa thế giới
sống qua gate thí nghiệm khi `WorldLawSet`, `ExperimentManifest`, `CausalLedger` và `SimClock` chưa
trở thành tài nguyên/luật của engine sống — mà đó đúng là nội dung §3.6. Một phiên nhận §3.3 mà bỏ
qua §3.6 sẽ chạm tường ngay. Chúng được đánh số ở hai bậc ưu tiên chỉ vì thứ tự trình bày;
**trên thực tế §3.6 cũng là P0.**

**Nhãn:** `DETERMINISM_CONTRACT` §5 và kế hoạch G0–G4 (dòng 852) gọi phần đường khởi động live này
là **G2**. Tài liệu này gọi nó là §3.3. Cùng một việc — giữ cả hai tên khi tra cứu chéo.

**Định nghĩa hoàn thành:** một manifest chạy được trên thế giới sống cho cùng checksum qua
hai lần chạy cùng seed · save/load giữ nguyên quỹ đạo · gỡ được câu cấm tương ứng trong CLAUDE.md.

---

### P1 — Nợ nền tảng đã biết

#### 3.4 Gate `burn`/`wgpu` sau feature flag

**Blocker là hình dạng code, không phải khối lượng.** `learn_handle` tại
[`src-tauri/src/core/simulation_loop.rs:182`](../../src-tauri/src/core/simulation_loop.rs)
được gán từ một `if has_wgpu { … } else { … }` mà hai nhánh **không `cfg` riêng từng nhánh được**.
Cần tái cấu trúc chỗ đó, không phải rắc attribute. `burn` chỉ dùng ở **hai** file — bảy kết quả
grep còn lại là văn xuôi khớp chữ "burned energy".

**Định nghĩa hoàn thành:** `cargo tree --no-default-features` không còn `burn-wgpu`/`naga` ·
gate `cargo tree` trong CI mở rộng để phủ chúng · fallback CPU vẫn chạy.

#### 3.5 Thu hẹp `tokio = { features = ["full"] }`

[`src-tauri/Cargo.toml:56`](../../src-tauri/Cargo.toml). Hai hệ con cần tokio đã nằm sau feature
(`networking`, `neo4j`), nên `full` giờ là thừa. Đây là bước tự nhiên tiếp theo sau G2 gate #2.

**Định nghĩa hoàn thành:** chỉ liệt kê feature thật sự dùng · cả hai cấu hình
(`--no-default-features` và `--features desktop`) đều clippy sạch và test xanh.

#### 3.6 G2 gate #1 — một thay đổi luật đổi cả hai engine

> **Thực chất là P0, không phải P1.** Đây là điều kiện tiên quyết của §3.3 — cùng là G2 task 1 /
> hội tụ AE4. Nó nằm dưới đề mục P1 vì thứ tự đánh số, không vì ưu tiên. Đừng nhận §3.3 mà bỏ mục này.

`crates/anima-domain` đã tách và đang giữ `causal`, `energy`, `intervention`, `laws`,
`sim_clock`, `units`. Gate đầy đủ đòi tách tiếp thành nhiều workspace member. Đây là mục
G2 duy nhất thật sự nhiều phiên.

**Định nghĩa hoàn thành:** một hằng số luật đổi ở `anima-domain` làm cả hai engine đổi hành vi,
có test chứng minh.

#### 3.7 Vòng đời thread: supervisor **xong**, cancellation token còn lại

**Cách đặt vấn đề cũ của mục này sai, và đó là phần đáng đọc nhất.** Nó nói "rò thread". Một lượt
kiểm kê **cả bảy** chỗ `thread::spawn` trong crate cho thấy handle nào cũng được thu hồi: năm cái vào
vector mà `stop` join, `inference_handle` được join trong sim thread, và hai biến thể learner là một
thread mà chỉ một cái được dựng. **Không có thread Rust nào bị rò.** Ai nhận mục này mà đi tìm chỗ rò
sẽ tìm một thứ không tồn tại.

Khoảng trống thật là `stop` không trả lời được hai câu hỏi duy nhất quan trọng khi shutdown sai — và
chế độ hỏng thì vô hạn.

##### Đã xong (2026-07-26, PR #14 → `e5fb8be`)

[`core/thread_supervisor.rs`](../../src-tauri/src/core/thread_supervisor.rs). Mỗi thread mang một
`ExitToken` có tên, **move vào closure** nên nó drop khi stack unwind — kể cả khi panic. Đó là phân
biệt mà `let _ = handle.join()` không làm được: thread chết bẩn báo "đã thoát", không phải "treo".

`stop` giờ **chờ báo cáo trước khi join**. Thứ tự là cốt lõi: `JoinHandle::join` không có timeout, nên
join trước nghĩa là một thread phớt lờ `running` làm `stop` chờ mãi — đúng hình dạng cú treo CI, một
cú chờ không giết được và không in gì.

Gate: [`tests/thread_lifecycle_tests.rs`](../../src-tauri/tests/thread_lifecycle_tests.rs) (6) +
6 unit test của module. Hợp đồng "sau `stop`, không còn gì `start` sinh ra đang chạy" nay **được máy
kiểm** thay vì giả định, cộng 8 chu kỳ start/stop vì lỗi CI xuất hiện ở test cycle 101 lần — nơi rò
một thread mỗi chu kỳ mới cộng dồn.

**Đánh đổi, ghi ra chứ không giấu:** thread quá hạn 30 giây thì **không** được join. Nó bị detach và
rò suốt đời tiến trình. Rò một thread **có tên** kèm thông báo hơn một `stop()` không bao giờ trả về
và không nói gì — nhưng đó là đánh đổi, không phải bản sửa. Bản sửa là thứ khiến thread đó phớt lờ
`running`.

##### Còn lại

- **Cancellation token thống nhất.** Hiện chỉ có một tín hiệu, `running: Arc<AtomicBool>`, và mỗi
  thread tự chọn tần suất kiểm. Supervisor cho biết *ai* không về; nó không làm thread đó về nhanh hơn.
- **Cú treo CI vẫn chưa được giải thích.** Nó không tái hiện kể từ PR #10, và phần vừa xong gỡ chế độ
  hỏng *vô hạn* mà **chưa** tái hiện được thứ kích hoạt nó. Ba dụng cụ giờ đã ở trên `main` (watchdog
  PR #10, `--nocapture` PR #12, supervisor PR #14); lần treo kế tiếp sẽ nêu tên chu kỳ, pha, hoặc
  thread — thay vì 90 phút im lặng.
- Closure sim-thread ~900 dòng và evo thread ~290 dòng trong `start()` vẫn chưa tách; xem §4 và ghi
  chú nợ của lượt refactor trước.

#### 3.8 Hai ADR đang ở trạng thái `proposed`, và cả hai đều đang đỡ tải

`proposed` chỉ vô hại khi chưa có gì dựa lên nó. Hiện không còn như vậy:

- **[ADR-0002](../decisions/ADR-0002-world-laws-and-exotic-energy.md)** — AE1–AE3 đã ship trên nó
  (xem §3.10). Điều mới: ADR-0004 viện dẫn quy tắc **ER01** của nó (`WorldLawSet` bất biến trong một
  run; nhánh checkpoint không đổi law fingerprint) làm ràng buộc thiết kế. Một ADR `proposed` giờ là
  nền của một ADR `proposed` khác — hoà giải §3.10 vì thế đã lên giá.
- **[ADR-0004](../decisions/ADR-0004-observer-as-declared-intervention.md)** — **accepted
  2026-07-26, O1 đã ship.** Người quan sát nhập vai là can thiệp được khai báo. Bối cảnh là một phát
  hiện chứ không phải đề xuất tính năng: `LodFocus` do camera lái **đã** là forcing lên thế giới và
  nằm ngoài mọi provenance, vì [`tier_at`](../../src-tauri/src/core/simulation_lod.rs) phân tier theo
  khoảng cách tới observer và
  [`cold_agents_stop_asking_entirely`](../../src-tauri/tests/simulation_lod_tests.rs) chốt rằng agent
  `Cold` **thật sự không suy nghĩ**.

  O1 đã đóng chỗ đó: [`core/observer.rs`](../../src-tauri/src/core/observer.rs) +
  enforcement ở `sync_lod_focus_system` + field `observer` trên `ExperimentManifest`
  (`#[serde(default)]`, **không** bump schema version). Gate:
  [`tests/observer_policy_tests.rs`](../../src-tauri/tests/observer_policy_tests.rs) — 7 pass, gồm
  `spectate_matches_absent` **và** control âm `an_inhabited_camera_actually_changes_who_thinks`;
  cộng 11 unit test cho kiểu và cho manifest.

  **Bẫy còn sống, đọc trước khi sửa quanh đây:** *thiếu resource `ObserverPolicy`* ≠ `Absent`. Thiếu
  nghĩa là chưa ai khai báo, và phải giữ nguyên hành vi cũ — **tuân theo camera**. `Absent` là khai
  báo ngược lại và **cấm** camera. Lẫn hai cái này sẽ âm thầm tắt LOD của app đang chạy, vì
  `PixiViewport.tsx` vẫn gọi `set_lod_focus` thật.

  **O2 cũng đã ship (2026-07-26).** `ObserverTrace` ghi focus **hiệu lực** (sau policy) vào buffer
  cấp phát sẵn, ghi-khi-đổi, và **đếm** mẫu tràn thay vì bỏ im lặng. `CAUSE_OBSERVER` nằm ở đỉnh dải
  `CauseId` vì scenario cấp tay từ dưới lên; manifest bị cấm giành id đó. App sống nay khai báo
  `Inhabit` thật thay vì "chưa khai báo" — hành vi không đổi, nhưng hệ quả đã có gốc.
  `DETERMINISM_CONTRACT` §2 nay là **năm** nguồn rò rỉ, có §2.1 cho camera.

  Gate: `tests/observer_trace_tests.rs` (6) + `tests/observer_trace_zero_alloc_tests.rs` (1 test,
  3 pha) — gồm control âm `a_chain_the_observer_did_not_start_does_not_name_them`.

  **Bẫy thứ hai, do chính test bắt được:** từ chối focus phải thay **trọn** `LodFocus::default()`,
  không chỉ tắt `enabled`. Giữ lại `center` sẽ để nguyên một đường camera sống bên trong world cho
  system sau này đọc phải — tái lập đúng cái nhiễu vừa cấm.

  **O3 — cơ chế đã ship (2026-07-26), tuyên bố phiên sống vẫn chặn.** `ObserverReplay` phát lại một
  trace thay cho camera sống và **loại trừ** camera chứ không xếp trên nó. Nội suy được khai báo:
  focus **giữ nguyên** giữa hai mẫu — không phải xấp xỉ, vì `record` chỉ lưu khi giá trị đổi. Gate:
  `tests/observer_replay_tests.rs` (6), gồm control âm và một test camera thù địch không lái được
  replay.

  Ranh giới, đọc kỹ trước khi tuyên bố gì: gate đó ghim **hệ con** và tự khai báo thứ tự schedule —
  đúng phạm vi mà `SNAPSHOT_CONTRACT` §8 tự nhận về mình. Gate
  `an_inhabited_run_replays_from_its_trace_without_a_human` đo *quỹ đạo thế giới sống* và vẫn
  **pending**: physics/CPG chạy song song nên một run liền mạch còn không khớp chính nó.

  **Chưa làm, có lý do:** lưu trace vào save state cần bump `SCHEMA_VERSION` 4→5, mà
  `MIN_SUPPORTED_SCHEMA = SCHEMA_VERSION - 2` nên bump sẽ **mất khả năng đọc save v2**. Trả cái giá
  đó cho dữ liệu chưa mode nào tiêu thụ là sai thứ tự. Khi làm, phải vào **cả** `SavedSimulationState`
  lẫn `world_checksum` một lượt (§8) — lưu ý khi *ghi* thì trace là đầu ra và không thuộc checksum,
  khi *phát lại* thì phần còn lại là đầu vào và thuộc.

  **Hành động nhập vai: đã ghi nhận (2026-07-26), chưa cưỡng chế.** Câu cũ ở đây nói engine chưa có
  hành động nhập vai nào. Sai — chúng đã tồn tại từ trước, chỉ không được gọi bằng tên đó. Bốn lệnh
  IPC ghi thẳng vào thế giới đang chạy mà không khai báo: `update_evolution_settings`,
  `toggle_evolution`, `trigger_migration`, `set_sharding_config`. Và chúng **mạnh hơn camera** —
  camera đổi *con nào được suy nghĩ*, còn cái đầu tiên đổi **luật mà chọn lọc vận hành dưới đó**,
  giữa run. Nghĩa là `DETERMINISM_CONTRACT` §2 chưa đủ ở nguồn thứ năm.

  Đã ship: `ObserverAction` + `SharedObserverActions` + `drain_observer_actions_system` + buffer
  `actions` riêng trong `ObserverTrace` (đếm tràn riêng, vì mất một hành động là lỗ **provenance**
  còn mất một mẫu focus chỉ mất độ trung thực replay). Gate:
  [`tests/observer_action_tests.rs`](../../src-tauri/tests/observer_action_tests.rs) (11), gồm một
  test **quét mã nguồn** chặn bốn lệnh đó lặng lẽ quay lại đường cũ — kèm control âm chứng minh scan
  có thể fail.

  Còn lại là **cưỡng chế**: một lệnh vẫn có thể với qua queue mà ghi thẳng shared state. Việc đó cần
  ledger có mặt trong world sống, tức §3.3/§3.6.

#### 3.15 Phả hệ: bộ nhớ không có trần, và không truy được dòng dõi

> **Số thứ tự là append-only.** Mục này thuộc **P1** nhưng mang số 3.15 vì 3.9–3.14 đã được bảng P2
> dùng, và `CLAUDE.md` cùng các kế hoạch khác tham chiếu chéo theo số. Đổi số sẽ làm gãy các tham
> chiếu đó — đắt hơn nhiều so với một số thứ tự trông lệch.

**Vì sao P1.** [`evolution/lineage.rs`](../../src-tauri/src/evolution/lineage.rs) lưu mỗi lần sinh
sản thành một `LineageNode` kèm **bản sao đầy đủ** `MorphologyGenotype`, cộng một `LineageRelation`
cho mỗi cha mẹ, và **không bao giờ prune**. Bộ nhớ vì thế tăng theo **tổng số cá thể từng sống**,
không theo số đang sống — với một run 60 FPS dài đó là đường tăng không có trần. Đây cũng là lý do
không trả lời được "hai cá thể này rẽ nhánh ở đâu": không có truy vấn MRCA.

Hệ quả khoa học đáng kể hơn hệ quả kỹ thuật: **không có MRCA thì không bám được *line of descent***,
tức là không dùng được giao thức mà Avida dùng để **đo** một sự kiện tiến hoá thay vì kể lại nó. Với
một dự án mà §3.3 đang cố chứng minh thế giới sống là experiment-ready, đó là một lỗ hổng bằng chứng.

**Điểm neo:** `InMemoryLineageTracker`, `LineageNode`, `LineageRelation`, trait `LineageTracker` —
tất cả trong [`evolution/lineage.rs`](../../src-tauri/src/evolution/lineage.rs). Test tải sẵn có:
`tests/lineage_stress.rs`.

**Đường đi, và nó không tốn dependency nào.** Chi tiết ở OS7 trong
[kế hoạch áp dụng nguồn mở](OPEN_SOURCE_ADOPTION_PLAN.md) và
[khảo sát §5](../research/OPEN_SOURCE_LANDSCAPE.md). Tóm tắt: lấy **thuật toán** `simplify()` của
tskit chứ không lấy crate (binding C + mô hình tree-sequence trên toạ độ genomic, cả hai đều không
hợp với genotype kiểu Karl Sims ở đây), và lấy **định dạng** Newick chứ không lấy code R.

**Thứ tự bắt buộc:** Newick → `simplify`/MRCA → giao thức đo. Không có MRCA thì không có gì để xuất
ra một cây có nghĩa.

**Định nghĩa hoàn thành:** một parser bên thứ ba (`ape`/DendroPy) đọc được output Newick · test từ
chối cây có chu trình, node mồ côi hoặc nhiều gốc · sau `simplify`, bộ nhớ lineage là O(cá thể
sống) và quan hệ tổ tiên của phần giữ lại **không đổi** · MRCA tất định, có test trên cây biết trước
đáp án.

---

### P2 — Vệ sinh, làm được lẻ

| # | Việc | Điểm neo | Vì sao đáng làm |
|---|---|---|---|
| 3.9 | Thêm `// SAFETY:` cho hai `unsafe impl Send/Sync`, hoặc bỏ nếu không còn cần | [`ai/model.rs:360`](../../src-tauri/src/ai/model.rs) | Đây là **2/2** khối unsafe của cả backend, và không cái nào có luận chứng. Type này ôm `WgpuDevice` |
| 3.10 | Hoà giải trạng thái ADR-0002 | [`ADR-0002`](../decisions/ADR-0002-world-laws-and-exotic-energy.md) vẫn `proposed` | AE1–AE3 đã ship. Theo quy tắc 6 của [chính sách tài liệu](../governance/DOCUMENTATION_POLICY.md), khi code và tài liệu xung đột thì **mở finding**, không tự coi code là đúng. **Đã lên giá:** ADR-0004 nay dựa vào ER01 của nó — xem §3.8 |
| 3.11 | Giảm 491 warning ESLint | `scripts/eslint_ratchet.mjs` | Ratchet chặn tăng nhưng không ép giảm. Nợ đang đóng băng, không co lại |
| 3.12 | Tách file lớn | `experiment_runner.rs` 3.173 dòng · `experiment.rs` 2.192 · `exotic_energy.rs` 1.839 | Tiền lệ tốt đã có: `aae673e` tách learner và emit thread khỏi `simulation_loop.rs` |
| 3.13 | Nợ phụ thuộc | `burn` 0.13.2 (ghim, có lý do ghi trong CLAUDE.md) · `bevy_ecs` 0.13 · React 18→19 · `@react-three/fiber` 8→9 · `three` 0.184→0.185 | **Không phải nợ bảo mật** — advisory đã sạch và đã có gate. Là nợ framework |
| 3.14 | Dọn tài liệu cũ ở root | [`handoff.md`](../../handoff.md), [`plan.md`](../../plan.md) | Mô tả công việc Phase 1 / Phase 6 đã xong từ lâu. Đã gắn nhãn lịch sử; bước sau là chuyển vào `docs/archive/` |
| 3.16 | Đóng phần còn nợ của OSS-003 | [`LICENSE`](../../LICENSE) proprietary đã có; **chưa có `NOTICE`** | `LICENSE` không tách phạm vi code / model / dataset / asset. Và một sản phẩm proprietary **vẫn** phải attribution cho mọi thành phần permissive được phân phối — hiện chưa có file nào làm việc đó |
| 3.17 | Inventory dependency (OSS-004) | [`OPEN_SOURCE_LANDSCAPE.md`](../research/OPEN_SOURCE_LANDSCAPE.md) | Lỗ hổng đã có bằng chứng: `burn`/`burn-wgpu` là **runtime dep đang chạy** nhưng vắng khỏi ma trận khảo sát tới 2026-07-26, trong khi ma trận có cả những thứ chưa từng thêm |

---

## 4. Bẫy đã biết — đọc trước khi sửa

Không lặp lại nội dung CLAUDE.md; đây là những cái tốn nhiều giờ nhất và dễ tái phát nhất.

- **Gate im lặng.** Một test target biên dịch mà không chạy test nào sẽ **exit 0**. Luôn chạy
  `check_test_targets.mjs` sau `cargo test`, và luôn truyền `--features desktop`.
- **Layout trọng số não là `w[out * fan_in + in]`** — chuyển vị so với `[d_input, d_output]` của Burn.
  Chép phẳng không chuyển vị thì mạng **vẫn chạy, vẫn ra số hữu hạn, và sai âm thầm**.
- **Mọi khoản trừ năng lượng mới phải vào `total_cost`** trong `metabolic_decay_system`. Một phép
  trừ riêng trông rất hợp lý và làm rò EU, vì chỉ `total_cost` chảy qua `respired` vào detritus.
- **`ActionGates` vắng mặt phải đọc là MỞ**, không bao giờ là đóng. Mặc định ngược lại sẽ âm thầm
  làm agent ngừng ăn.
- **Không Lamarck.** `AgentBrain.learned` không bao giờ được ghi ngược vào `.genotype`.
- **Code số học cần HAI loại test.** Gradient check bằng sai phân hữu hạn bắt được đạo hàm sai,
  nhưng **vẫn pass với một mục tiêu sai**. Phải ghép với một khẳng định về hành vi.
- **Test đọc/ghi biến môi trường phải chia sẻ một mutex.** Chúng chạy song song trong cùng tiến trình;
  xanh khi chạy riêng file, đỏ khi chạy cả suite.
- **`terrain_challenger_tests` zero-alloc từng flaky** khi bốn test trong file chạy song song
  (lệch đúng 1 allocation). Lần chạy 2026-07-26 xanh; nếu thấy đỏ, chạy riêng file trước khi
  kết luận là hồi quy.
- **Chạy `cargo` từ PowerShell, không phải Git Bash.** Đo 2026-07-26: cùng một lệnh, cùng một cây mã
  nguồn — qua Git Bash cho `587 passed, 0 failed` với **15 target chết ở `STATUS_ENTRYPOINT_NOT_FOUND`
  (0xc0000139)** trước khi chạy nổi một test; qua PowerShell cho **629 passed, 0 failed, exit 0**.
  15 target đó đúng là nhóm feature-gated nạp DLL native nặng (`migration_*`, `persistence_*`,
  `lineage_stress`, `environmental_elements_*`, `tauri_ipc`, `adversarial_challenger`): Git Bash chèn
  `/usr/bin` của MSYS2 vào `PATH` và chúng phân giải nhầm một DLL ở đó. 0xc0000139 nghĩa là "tìm thấy
  DLL, thiếu entry point mong đợi".
  **Hai ngõ cụt, ghi ra để không ai đi lại:** đây *không* phải artifact cũ — `cargo clean -p anima-engine`
  rồi dựng lại tái hiện đủ 15 crash y hệt. Và *không* phải hồi quy — cây mã nguồn khi đó giống hệt một
  `main` vừa đo xanh vài giờ trước.
- **Chạy các suite tuần tự, đừng chồng nhau.** Một bản build `cargo` chạy song song làm suite Vitest
  `tests/` báo ~28 lỗi giả: timeout render bị đọc thành "không tìm thấy DOM node", nên lỗi trông như
  một khẳng định sai chứ không như một timeout. Chạy lại lúc máy rảnh: 243 pass, và thời gian tường
  rơi từ 50,5s xuống 19,5s. Trước khi kết luận đỏ là hồi quy, hãy hỏi máy lúc đó đang chạy gì.
- **Nhiều agent có thể cùng sửa cây này.** Một `cargo check` gãy giữa chừng có thể là bản ghi dở
  của phiên khác, không phải lỗi của bạn.
- **Và hệ quả nặng hơn: công việc của phiên khác có thể đang nằm trên nhánh của bạn.** Đã xảy ra
  ngày 2026-07-26 — toàn bộ tài liệu kiểm toán (`STATE_OF_THE_PROJECT.md`, `CLAUDE.md`, `TODO.md`,
  `handoff.md`, `plan.md`) chưa commit và nằm trên `fix/temp-path-collisions`, một nhánh có PR #6
  đang mở với tiêu đề chỉ nói về temp path. **Chạy `git status` và `gh pr list` trước khi commit**,
  và kiểm nhánh hiện tại có khớp việc mình đang làm không. Một PR đúng nội dung nhưng sai tiêu đề là
  một PR không ai review đúng.
- **Phiên khác `checkout` nhánh của họ *từ nhánh của bạn*, và việc chưa commit của bạn đi theo.**
  Đây không phải biến thể của mục trên — nó là cơ chế khác và nguy hiểm hơn, vì nó xảy ra **giữa lúc
  bạn đang làm** chứ không phải trước khi bạn bắt đầu. Reflog ghi thẳng ra:
  `checkout: moving from fix/thread-supervisor-3-7 to docs/oss-010-status-reconcile`.

  Hai lần trong một phiên ngày 2026-07-26, và **cách phòng hiển nhiên không đủ**:

  1. Lần một, việc chưa commit trôi sang nhánh của phiên khác **trong lúc bốn file của họ đang
     `staged`**. Một lệnh `git commit` ở thời điểm đó sẽ quét trọn việc của họ vào commit của mình.
  2. Lần hai, tôi đã kiểm nhánh ở đầu lệnh — nhưng nhánh bị đổi **giữa lệnh kiểm và lệnh commit,
     trong cùng một khối**, nên commit rơi vào `feat/oss-070-newick-export`. Kiểm rồi tin là không đủ.

  **Cách phòng thật sự hiệu quả là guard có abort, không phải kiểm rồi tin:**

  ```powershell
  $want = "your-branch"
  $have = git branch --show-current
  if ($have -ne $want) { "ABORT: on '$have', expected '$want'"; exit 1 }
  ```

  Và **stage theo từng đường dẫn, không bao giờ `git add -A`** — đó là thứ giữ cho việc của người khác
  không bị cuốn vào ngay cả khi guard trượt.

  Nếu commit đã rơi sai chỗ: `git cherry-pick <sha>` sang nhánh đúng, rồi `git branch -f <nhánh-của-họ>
  <sha-cũ>` để trả con trỏ về. Dùng `branch -f` chứ **không** `reset --hard` — reset sẽ xoá working tree
  mà phiên khác đang viết vào. Kiểm `git ls-remote --heads origin <nhánh>` trước: nếu nhánh đó chưa
  được push và chưa có commit nào của họ thì sửa hoàn toàn sạch.

  Nếu việc của bạn đang lẫn trong tree của họ: `git stash push --include-untracked -- <đúng các đường
  dẫn của bạn>`. Có pathspec thì nó không đụng index của họ; `git stash` trần thì có.
- **Bài học đầy đủ hơn cả hai mục trên: trong checkout dùng chung, đừng dựa vào `HEAD` cho bất cứ
  việc gì.** Guard ở trên chặn được commit sai nhánh, nhưng nó không chặn được mọi thứ:

  - `git switch main` **im lặng không có hiệu lực** khi phiên khác vừa chiếm `HEAD` — không lỗi,
    không cảnh báo, và mọi lệnh sau đó chạy trên nhánh của họ.
  - `git pull --ff-only` khi đó fail với `Not possible to fast-forward` vì bạn đang trên nhánh đã
    phân kỳ của họ, chứ không phải `main`.
  - `git branch -d <nhánh>` so với **remote-tracking ref**, không phải với `main`, rồi cảnh báo
    "not yet merged to HEAD". Nó vẫn xoá. Cái thật sự bảo vệ bạn là kiểm bằng ref tường minh trước:
    `git log origin/main..<nhánh>` phải trống, hoặc `git merge-base --is-ancestor <sha> origin/main`.

  Nên: **dùng ref tường minh (`origin/main`, sha) cho mọi phép kiểm**, và khi cần một `HEAD` mà
  không ai giành được, dùng **worktree** thay vì `git switch`:

  ```powershell
  git worktree add -b <nhánh-của-bạn> <đường-dẫn-tạm> origin/main
  # ... làm việc, commit, push trong đó ...
  git worktree remove <đường-dẫn-tạm>
  ```

  Worktree có `HEAD` và working tree riêng, nên nó **không** chạm checkout dùng chung — không kéo theo
  việc chưa commit của phiên khác, và không đặt commit của bạn lên nhánh của họ. Mục §4 này được viết
  chính bằng cách đó, sau khi `git switch` thất bại trong im lặng hai lần.

---

## 5. Lệnh xác minh đầy đủ

Chạy từ repo root, trừ chỗ ghi khác. Đây là đúng bộ CI chạy.

```bash
npm run lint && node scripts/eslint_ratchet.mjs && npm run test && npm run test:frontend && npm run build && node scripts/check_docs_links.mjs
```

```bash
cd src-tauri && cargo fmt --check && cargo clippy --all-targets --features desktop -- -D warnings && cargo test --features desktop
```

Skill [`verify-anima`](../../.claude/skills) gói backend + frontend + build vào một lượt.
