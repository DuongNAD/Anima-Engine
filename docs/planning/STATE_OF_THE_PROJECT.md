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
| Link tài liệu | `node scripts/check_docs_links.mjs` | 245 link, **0 gãy** |

Quy mô: Rust ~47,7k dòng / 128 file · TS ~25,6k dòng / 126 file · 627 hàm `#[test]` ·
62 file test tích hợp backend · 46 file test frontend · 7 spec Playwright · 45 tài liệu.

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

#### 3.2 Thay số hiệu năng proxy bằng số đo thật

**Vì sao P0.** [`BENCHMARK_BASELINE.md`](../../BENCHMARK_BASELINE.md) **tự khai** rằng số hiện
tại là proxy, vì chạy full backend đã crash máy dev. Nghĩa là tuyên bố "60 FPS real-time"
của dự án **chưa từng được đo**. Mọi quyết định về scale (LOD, ngân sách bộ nhớ não, số agent
thường trú) đang dựa trên ước lượng.

**Ràng buộc vận hành có thật:** không chạy `npm run tauri dev` / `cargo run` trên máy dev.
Đây là ràng buộc, không phải lời khuyên — nó đã crash máy.

**Đường đi khả thi:** `scripts/bench_baseline.mjs` đã chạy hoàn toàn trên CPU, không mở
device, xong dưới một giây. Cần một bản chạy **trên phần cứng đích** (hoặc một runner có GPU)
để thay các trường proxy, rồi commit report kèm khối hardware.

**Định nghĩa hoàn thành:** `benchmark_report.json` chứa số đo thật kèm định danh phần cứng ·
test S04 (`src/__tests__/benchmarkReport.test.ts`) vẫn xanh · `BENCHMARK_BASELINE.md` gỡ nhãn proxy ·
ngân sách của EB-S12 (22,5 KiB/agent, ~46.500 agent/GiB) được đối chiếu với số thật.

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

#### 3.7 Vòng đời thread: supervisor + cancellation

`simulation_loop.rs` spawn bốn thread (`evo`, `sim`, `inference`, `net`), `emit.rs` và
`training.rs` mỗi file thêm nữa. **Lưu ý bản ghi cũ đã lạc hậu:** `inference_handle` từng bị
drop thì **nay đã được join** ([`simulation_loop.rs:660`](../../src-tauri/src/core/simulation_loop.rs)
có ghi chú giải thích). Phần còn thiếu là supervisor và cancellation token thống nhất, không
phải cái handle đó.

#### 3.8 Hai ADR đang ở trạng thái `proposed`, và cả hai đều đang đỡ tải

`proposed` chỉ vô hại khi chưa có gì dựa lên nó. Hiện không còn như vậy:

- **[ADR-0002](../decisions/ADR-0002-world-laws-and-exotic-energy.md)** — AE1–AE3 đã ship trên nó
  (xem §3.10). Điều mới: ADR-0004 viện dẫn quy tắc **ER01** của nó (`WorldLawSet` bất biến trong một
  run; nhánh checkpoint không đổi law fingerprint) làm ràng buộc thiết kế. Một ADR `proposed` giờ là
  nền của một ADR `proposed` khác — hoà giải §3.10 vì thế đã lên giá.
- **[ADR-0004](../decisions/ADR-0004-observer-as-declared-intervention.md)** (2026-07-26, chờ quyết
  định) — người quan sát nhập vai là can thiệp được khai báo. **Bối cảnh là một phát hiện, không phải
  một đề xuất tính năng:** `LodFocus` do camera lái **đã** là một forcing lên thế giới và đang nằm
  ngoài mọi provenance. [`tier_at`](../../src-tauri/src/core/simulation_lod.rs) phân tier theo khoảng
  cách tới observer, và [`cold_agents_stop_asking_entirely`](../../src-tauri/tests/simulation_lod_tests.rs)
  chốt rằng agent `Cold` **thật sự không suy nghĩ**. Nghĩa là chỗ người dùng nhìn quyết định con nào
  được suy nghĩ. Hôm nay run nghiên cứu vẫn sạch, nhưng chỉ vì `LodFocus::default()` là
  `enabled: false` và headless không có camera — đó là **hệ quả phụ của việc chưa có UI**, không phải
  một hợp đồng.

  Nếu ADR được chấp nhận, hạng mục **O1** (`ObserverPolicy::Absent`/`Spectate` + gate
  `spectate_matches_absent`, kèm control âm) làm được **ngay** và **không** phụ thuộc §3.3/§3.6.
  Các hạng mục còn lại (ghi trace, replay `Inhabit`) thì phụ thuộc.

  Lưu ý cho người áp dụng: ADR còn `proposed`, nên **chưa** được đem hệ quả của nó vào
  `DETERMINISM_CONTRACT` hay `SNAPSHOT_CONTRACT`. Nó có ghi rõ các contract sẽ bị ảnh hưởng nếu được
  chấp nhận.

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
