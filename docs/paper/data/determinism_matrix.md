---
title: Ma trận tái lập đa nền tảng (M1)
status: active
owner: maintainers
last_reviewed: 2026-11-19
review_cycle: per-release
---

# Ma trận tái lập đa nền tảng (thẻ M1)

**Câu hỏi.** Cùng seed và cùng manifest có cho cùng một thế giới trên Windows, Linux và macOS không?
Trước lượt đo này, mọi gate tất định của dự án chỉ so các lượt chạy trên **một** máy.

## Kết quả, đúng như đo được

| | Lúc genesis (tick 0) | Từ tick 180 trở đi |
|---|---|---|
| **Thế giới live, địa hình sinh tại chỗ** | **Khác** giữa cả ba hệ điều hành: mỗi máy sinh ra một địa hình khác (checksum địa hình 22481267 / 3152323152 / 1212187560) | Khác |
| **Thế giới live, artifact `artifacts/world_256.anmw` đã commit** | **Giống** trên cả ba: 6/6 điểm | **Khác**: lệch đầu tiên ở năng lượng động vật (`detritus`, `live.animals_eu`, `live.mean_agent_energy`) |
| **`ReferenceEvolutionWorld`** (engine tổng hợp) | (không có hàng tick 0) | **Giống** trên cả ba: 9/9 điểm, tới tick 18.000 |

Ba điều rút ra, theo đúng mức bằng chứng:

1. **Có hai nguồn lệch tách biệt.** Bộ sinh địa hình lệch theo hệ điều hành. Khi ghim địa hình bằng
   artifact, nguồn này biến mất, nhưng động lực học vẫn lệch trước tick 180.
2. **Lệch theo hệ điều hành, không theo phần cứng.** Linux x86_64 và Windows x86_64, cùng kiến trúc,
   vẫn lệch nhau. macOS trên runner CI và macOS trên một máy Apple M5 cục bộ thì trùng cả 27/27 điểm
   ở lượt đo đầu. Cả ba runner dùng **cùng một rustc 1.98.1** (ghim trong `rust-toolchain.toml`).
3. **Engine reference không có lời gọi hàm siêu việt nào** (`sin`, `cos`, `exp`, `powf`, `ln`…) và
   trùng tuyệt đối. Thế giới live có các lời gọi đó ở cả bộ sinh địa hình (`terrain.rs`: `powf(1.15)`
   cho độ cao) lẫn mỗi tick (`ecology.rs`: `exp` và `powf` trong tốc độ trao đổi chất MTE/Kleiber;
   `cpg.rs` và mùa: `sin`). Rust chuyển các hàm này cho libm của nền tảng (glibc, MSVC CRT, Apple
   libm), và chúng không bắt buộc trả về cùng một bit.
4. **Chính các hàm đó trả về khác nhau, đã đo trực tiếp** (bảng [Các hàm toán](#các-hàm-toán)). Với
   cùng 2.048 input:
   - `sqrt`, đối chứng mà IEEE 754 bắt buộc làm tròn đúng, và `ln` trùng tuyệt đối;
   - `sin`, `cos`, `exp`, `powf`, `atan2` và `tanh` trả về khác nhau ở một phần input, luôn đúng
     **1 ULP**. `f32::sin` lệch ở 289/2048 input; `f32::powf(x, 1.15)` của bộ sinh địa hình lệch ở
     19/2048.

   Phần **chưa** được chứng minh là quan hệ nhân quả cụ thể: lượt đo không thay hàm nào trong mô phỏng
   để xem thế giới có hết lệch không. Nhưng mọi điều đo được đều khớp với cách giải thích này, và
   không điều nào chống lại nó.

## Các hàm toán

Run `44040775729` của workflow `determinism-matrix`, commit `abfccf2` (nhánh
`swarm/m1-libm-fingerprint`), cùng ba runner và cùng rustc 1.98.1. Test
`m1_print_libm_result_bits_for_cross_platform_comparison` in bit kết quả cho 2.048 input sinh bằng
LCG, nên giống hệt nhau trên mọi nền tảng. Khoảng giá trị bám theo thứ mô phỏng thực sự truyền vào:
- pha của CPG và của mùa;
- số mũ Boltzmann;
- `m^0,75` của Kleiber;
- `elevation^1,15` của địa hình trên [0, 1].

"ULP" là khoảng cách giữa hai kết quả, tính bằng số giá trị float liền kề.

| Hàm | Input | Input lệch bit giữa các nền tảng | Lệch lớn nhất (ULP) |
|---|---:|---:|---:|
| `f32::sqrt` (đối chứng) | 2048 | 0 | 0 |
| `f32::sin` | 2048 | 289 | 1 |
| `f32::cos` | 2048 | 30 | 1 |
| `f32::exp` | 2048 | 19 | 1 |
| `f32::ln` | 2048 | 0 | 0 |
| `f32::powf(x, 0.75)` | 2048 | 7 | 1 |
| `f32::powf(x, 1.15)` | 2048 | 19 | 1 |
| `f32::atan2(x, 1.5)` | 2048 | 3 | 1 |
| `f32::tanh` | 2048 | 41 | 1 |
| `f64::sqrt` (đối chứng) | 2048 | 0 | 0 |
| `f64::sin` | 2048 | 127 | 1 |
| `f64::cos` | 2048 | 148 | 1 |
| `f64::exp` | 2048 | 9 | 1 |
| `f64::ln` | 2048 | 0 | 0 |
| `f64::powf(x, 0.75)` | 2048 | 5 | 1 |

Lệch 1 ULP là nhỏ nhất có thể, nhưng mô phỏng là một hệ hỗn loạn: ma trận ở trên cho thấy một bit
khác ở `powf` lúc sinh địa hình đủ đổi biome của vài ô, và vài bit khác ở trao đổi chất đủ đổi quỹ
đạo trước tick 180.

## Hệ quả

- **Checksum chỉ so được trong cùng một hệ điều hành.** Quy tắc "không ghim checksum tuyệt đối trong
  test, so hai lượt chạy với nhau" giờ có số đo đỡ. Một kết quả thí nghiệm trên thế giới live chỉ tái
  lập được bit-for-bit trên cùng hệ điều hành; giữa các hệ điều hành chỉ so được ở mức thống kê.
- **Thế giới live lấy địa hình từ môi trường của máy — ĐÃ SỬA CHO ĐƯỜNG CHẠY HEADLESS (thẻ F051, PR #174).**
  Trước thẻ F051, `core::ecs::init_world` nạp `ANIMA_WORLD_ARTIFACT`, hoặc `$TMPDIR/anima_world_artifact.anmw`
  (file app desktop ghi khi frontend sinh thế giới), rồi mới rơi về sinh tại chỗ và cache vào
  `$TMPDIR/anima_world_cache`. Lúc genesis (`live.probe`, seed 1), artifact đã commit cho 258.450 EU
  thực vật trên cả ba runner; địa hình sinh tại chỗ cho 153.688 (Linux), 153.587 (Windows) và 154.972
  (macOS), nên một máy từng mở app chạy thí nghiệm headless trên thế giới khác hẳn máy sạch (còn
  `artifacts/experiments/e2-evolved-brain-default` và các bản chạy cũ trước F051 vẫn giữ nguyên).
  Từ thẻ F051:
  - Mọi runner/adapter headless (`build_live_world` → `core::ecs::init_world_with(TerrainSource::Generated)`)
    luôn sinh địa hình trực tiếp từ `MapSettings::default()`, bỏ qua hoàn toàn `ANIMA_WORLD_ARTIFACT`,
    `<temp_dir>/anima_world_artifact.anmw` và `ANIMA_CACHE_DIR`, đồng thời ghi `terrain_source: "generated"`
    và `world_identity` quan sát được vào `RunProvenance` cũng như `provenance.json` của các runner
    (`run_esel1_experiment`, `run_sel2p_experiment`).
  - Đường chạy app desktop (`core::ecs::init_world` → `TerrainSource::AppArtifactOrGenerated`) **giữ
    nguyên** hành vi đọc shared world artifact để không phá đồng bộ với renderer.
  - Xem thêm bài học **E2-F2** (manifest không ghim được thế giới); E2 đã giảm nhẹ bằng cách ghi
    `WorldIdentity` quan sát được vào `provenance.json` (gate E2-G6).
- **Kết quả trên `ReferenceEvolutionWorld` (AE1–AE3) mang được sang nền tảng khác** ở mức bit, trong
  phạm vi lượt đo này.

Thẻ này **không sửa gì**, đúng như thẻ quy định. Hai hướng sửa đều là quyết định của chủ dự án, vì cả
hai đổi checksum của mọi thế giới live:

- **ghim thế giới** trong manifest (đường dẫn hoặc hash của artifact) và để adapter từ chối khi thế giới
  dựng ra không khớp; việc này cũng đóng E2-F2;
- **thư viện toán tất định** cho mã mô phỏng, ví dụ một bản libm thuần Rust, thay cho libm của nền tảng.
  Bảng trên cho biết phạm vi phải thay: `sin`, `cos`, `exp`, `powf`, `atan2`, `tanh` (không cần thay
  `sqrt` hay các phép tính cơ bản).

## Đo thế nào

- **Commit:** `eaf00356becb9d070b37cf3c1e3384f82753c306` (nhánh `swarm/m1-determinism-matrix`, dựa trên
  thân cây `8a4a89e`), ngày 2026-11-17.
- **Workflow:** `.github/workflows/determinism-matrix.yml`, run `44033468746`. Runner `ubuntu-26.04`,
  `windows-2025-vs2026`, `macos-26-arm64`; rustc 1.98.1 trên cả ba.
- **Probe:** `src-tauri/tests/cross_platform_checksum_probe.rs` với `ANIMA_M1_FULL=1`, bản release,
  feature mặc định. Seed 1, 2, 3; tick 0, 180, 1.800 và 18.000.
- **Ba mô hình:** `live.probe` (thế giới của `live_cross_process_probe.rs`), `live.p1` (cấu hình P1 có
  lái, chết và sinh sản) và `reference`.
- **Mỗi hệ điều hành chạy hai lượt:** một lượt sinh địa hình tại chỗ, một lượt với
  `ANIMA_WORLD_ARTIFACT` trỏ vào artifact đã commit.
- **So sánh:** `scripts/paper/compare_checksums.mjs` (có `--self-test` với năm đối chứng âm). Một điểm
  "giống" nghĩa là cùng checksum **và** cùng chuỗi `{:?}` cho mọi observable được in, tức là cùng
  bit.
- **Observable khối lượng cơ thể (`live.trait_mass_*`) không được in**, vì dòng thí nghiệm về khối
  lượng vẫn phải mù cho tới khi có bản đăng ký trước mới. Checksum vẫn phủ trạng thái đó. Vì vậy vài
  hàng ở dưới khác checksum mà không có observable được in nào khác: phần lệch nằm ở trạng thái không
  được in.

Chạy lại: Actions → `determinism-matrix` → *Run workflow*. Cục bộ, cho một hệ điều hành:

```bash
ANIMA_M1_FULL=1 cargo test --release --test cross_platform_checksum_probe -- --nocapture > m1.txt 2>&1
```

## Ma trận đầy đủ

Sinh bởi `compare_checksums.mjs` trong job `compare` của run `44033468746`, không sửa tay.

### Địa hình sinh tại chỗ trên mỗi runner

Platforms: `linux-x86_64`, `windows-x86_64`, `macos-aarch64` · mode `full` · 33 points

**9 of 33 points agree.** First difference (ordered by tick, then model, then seed): `live.p1` seed 1 at tick 0, checksums differ; observables that differ: `plants`, `live.closed_eu_total`, `live.standing_crop`, `world.terrain_checksum`.

| Model | Seed | Tick | Checksum `linux-x86_64` | Checksum `windows-x86_64` | Checksum `macos-aarch64` | Agree | Observables that differ |
|---|---|---|---|---|---|---|---|
| `live.p1` | 1 | 0 | 3012240515 | 2020381237 | 1624802853 | **no** | `plants`, `live.closed_eu_total`, `live.standing_crop`, `world.terrain_checksum` |
| `live.p1` | 2 | 0 | 716299376 | 2856665478 | 1350727926 | **no** | `plants`, `live.closed_eu_total`, `live.standing_crop`, `world.terrain_checksum` |
| `live.p1` | 3 | 0 | 2852310945 | 4145230199 | 509807591 | **no** | `plants`, `live.closed_eu_total`, `live.standing_crop`, `world.terrain_checksum` |
| `live.probe` | 1 | 0 | 3012240515 | 2020381237 | 1624802853 | **no** | `plants`, `live.closed_eu_total`, `live.standing_crop`, `world.terrain_checksum` |
| `live.probe` | 2 | 0 | 716299376 | 2856665478 | 1350727926 | **no** | `plants`, `live.closed_eu_total`, `live.standing_crop`, `world.terrain_checksum` |
| `live.probe` | 3 | 0 | 2852310945 | 4145230199 | 509807591 | **no** | `plants`, `live.closed_eu_total`, `live.standing_crop`, `world.terrain_checksum` |
| `live.p1` | 1 | 180 | 1635279574 | 1651569322 | 3765641225 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.standing_crop`, `live.mean_agent_energy`, `world.terrain_checksum` |
| `live.p1` | 2 | 180 | 2689563942 | 3040055350 | 2115745499 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.standing_crop`, `live.mean_agent_energy`, `world.terrain_checksum` |
| `live.p1` | 3 | 180 | 3287955098 | 3872333552 | 1634376100 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.standing_crop`, `live.mean_agent_energy`, `world.terrain_checksum` |
| `live.probe` | 1 | 180 | 2083822889 | 694568034 | 3685254391 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.standing_crop`, `live.mean_agent_energy`, `world.terrain_checksum` |
| `live.probe` | 2 | 180 | 1135772847 | 1943821048 | 4043307074 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.standing_crop`, `live.mean_agent_energy`, `world.terrain_checksum` |
| `live.probe` | 3 | 180 | 2624434800 | 2494771578 | 1116209104 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.standing_crop`, `live.mean_agent_energy`, `world.terrain_checksum` |
| `reference` | 1 | 180 | 963373296 | 963373296 | 963373296 | yes | — |
| `reference` | 2 | 180 | 1923432925 | 1923432925 | 1923432925 | yes | — |
| `reference` | 3 | 180 | 2593990385 | 2593990385 | 2593990385 | yes | — |
| `live.p1` | 1 | 1800 | 4116302365 | 1335878400 | 3594999398 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.agent_count`, `live.herbivore_count`, `live.standing_crop`, `live.mean_agent_energy`, `live.shannon`, `live.simpson`, `live.births`, `live.deaths`, `live.mean_lifespan`, `world.terrain_checksum` |
| `live.p1` | 2 | 1800 | 4243444245 | 230428829 | 1977189649 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.agent_count`, `live.herbivore_count`, `live.standing_crop`, `live.mean_agent_energy`, `live.shannon`, `live.simpson`, `live.births`, `live.mean_lifespan`, `world.terrain_checksum` |
| `live.p1` | 3 | 1800 | 3275013170 | 3183216432 | 2682781860 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.agent_count`, `live.herbivore_count`, `live.standing_crop`, `live.mean_agent_energy`, `live.shannon`, `live.simpson`, `live.births`, `live.mean_lifespan`, `world.terrain_checksum` |
| `live.probe` | 1 | 1800 | 3362116828 | 3410815382 | 4098211099 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.standing_crop`, `live.mean_agent_energy`, `world.terrain_checksum` |
| `live.probe` | 2 | 1800 | 4068710412 | 906029834 | 1486699401 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.standing_crop`, `live.mean_agent_energy`, `world.terrain_checksum` |
| `live.probe` | 3 | 1800 | 2431458118 | 416774673 | 2464264534 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.standing_crop`, `live.mean_agent_energy`, `world.terrain_checksum` |
| `reference` | 1 | 1800 | 1203614470 | 1203614470 | 1203614470 | yes | — |
| `reference` | 2 | 1800 | 956307915 | 956307915 | 956307915 | yes | — |
| `reference` | 3 | 1800 | 2493242080 | 2493242080 | 2493242080 | yes | — |
| `live.p1` | 1 | 18000 | 813275991 | 2628061208 | 206746169 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.agent_count`, `live.herbivore_count`, `live.standing_crop`, `live.mean_agent_energy`, `live.births`, `live.deaths`, `live.mean_lifespan`, `world.terrain_checksum` |
| `live.p1` | 2 | 18000 | 1127173894 | 4220448543 | 3141535070 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.agent_count`, `live.herbivore_count`, `live.standing_crop`, `live.mean_agent_energy`, `live.births`, `live.deaths`, `live.mean_lifespan`, `world.terrain_checksum` |
| `live.p1` | 3 | 18000 | 3487767856 | 351948251 | 3871222952 | **no** | `plants`, `detritus`, `live.closed_eu_total`, `live.standing_crop`, `live.births`, `live.deaths`, `live.mean_lifespan`, `world.terrain_checksum` |
| `live.probe` | 1 | 18000 | 1054901718 | 2601272375 | 1460036535 | **no** | `plants`, `detritus`, `live.closed_eu_total`, `live.standing_crop`, `world.terrain_checksum` |
| `live.probe` | 2 | 18000 | 2880640310 | 3062307791 | 687374 | **no** | `plants`, `detritus`, `live.closed_eu_total`, `live.standing_crop`, `world.terrain_checksum` |
| `live.probe` | 3 | 18000 | 3296653213 | 1268108105 | 2589012376 | **no** | `plants`, `detritus`, `live.closed_eu_total`, `live.standing_crop`, `world.terrain_checksum` |
| `reference` | 1 | 18000 | 2399239161 | 2399239161 | 2399239161 | yes | — |
| `reference` | 2 | 18000 | 487902251 | 487902251 | 487902251 | yes | — |
| `reference` | 3 | 18000 | 1244584487 | 1244584487 | 1244584487 | yes | — |

### Artifact đã commit (`artifacts/world_256.anmw`)

Platforms: `linux-x86_64`, `windows-x86_64`, `macos-aarch64` · mode `full` · 33 points

**15 of 33 points agree.** First difference (ordered by tick, then model, then seed): `live.p1` seed 1 at tick 180, checksums differ; observables that differ: `detritus`, `live.animals_eu`, `live.mean_agent_energy`.

| Model | Seed | Tick | Checksum `linux-x86_64` | Checksum `windows-x86_64` | Checksum `macos-aarch64` | Agree | Observables that differ |
|---|---|---|---|---|---|---|---|
| `live.p1` | 1 | 0 | 1723404533 | 1723404533 | 1723404533 | yes | — |
| `live.p1` | 2 | 0 | 2155250502 | 2155250502 | 2155250502 | yes | — |
| `live.p1` | 3 | 0 | 3848253495 | 3848253495 | 3848253495 | yes | — |
| `live.probe` | 1 | 0 | 1723404533 | 1723404533 | 1723404533 | yes | — |
| `live.probe` | 2 | 0 | 2155250502 | 2155250502 | 2155250502 | yes | — |
| `live.probe` | 3 | 0 | 3848253495 | 3848253495 | 3848253495 | yes | — |
| `live.p1` | 1 | 180 | 171284829 | 1591708224 | 3041340150 | **no** | `detritus`, `live.animals_eu`, `live.mean_agent_energy` |
| `live.p1` | 2 | 180 | 1232201150 | 101589807 | 2379431503 | **no** | `detritus`, `live.animals_eu`, `live.mean_agent_energy` |
| `live.p1` | 3 | 180 | 1250066937 | 1346617023 | 231926291 | **no** | `detritus`, `live.animals_eu`, `live.mean_agent_energy` |
| `live.probe` | 1 | 180 | 2149989286 | 2781638105 | 2518453240 | **no** | `detritus`, `live.animals_eu`, `live.mean_agent_energy` |
| `live.probe` | 2 | 180 | 1227642466 | 1145031147 | 1986018248 | **no** | — |
| `live.probe` | 3 | 180 | 4155039397 | 2364043734 | 289593694 | **no** | `detritus`, `live.animals_eu`, `live.mean_agent_energy` |
| `reference` | 1 | 180 | 963373296 | 963373296 | 963373296 | yes | — |
| `reference` | 2 | 180 | 1923432925 | 1923432925 | 1923432925 | yes | — |
| `reference` | 3 | 180 | 2593990385 | 2593990385 | 2593990385 | yes | — |
| `live.p1` | 1 | 1800 | 213395715 | 2196890120 | 3040303304 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.standing_crop`, `live.mean_agent_energy` |
| `live.p1` | 2 | 1800 | 566848000 | 3981098760 | 523680926 | **no** | `detritus`, `live.animals_eu`, `live.mean_agent_energy` |
| `live.p1` | 3 | 1800 | 741350026 | 2786361204 | 1932045404 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.agent_count`, `live.herbivore_count`, `live.standing_crop`, `live.mean_agent_energy`, `live.shannon`, `live.simpson`, `live.births`, `live.mean_lifespan` |
| `live.probe` | 1 | 1800 | 3470034063 | 1882182195 | 2742884439 | **no** | `detritus`, `live.animals_eu`, `live.mean_agent_energy` |
| `live.probe` | 2 | 1800 | 305453219 | 2456456843 | 3140640729 | **no** | — |
| `live.probe` | 3 | 1800 | 1972614158 | 3474601719 | 1067324835 | **no** | `detritus`, `live.animals_eu`, `live.mean_agent_energy` |
| `reference` | 1 | 1800 | 1203614470 | 1203614470 | 1203614470 | yes | — |
| `reference` | 2 | 1800 | 956307915 | 956307915 | 956307915 | yes | — |
| `reference` | 3 | 1800 | 2493242080 | 2493242080 | 2493242080 | yes | — |
| `live.p1` | 1 | 18000 | 4282661456 | 2051371596 | 1632848630 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.agent_count`, `live.herbivore_count`, `live.standing_crop`, `live.mean_agent_energy`, `live.births`, `live.deaths`, `live.mean_lifespan` |
| `live.p1` | 2 | 18000 | 256503819 | 87827033 | 942597110 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.agent_count`, `live.herbivore_count`, `live.standing_crop`, `live.mean_agent_energy`, `live.births`, `live.deaths`, `live.mean_lifespan` |
| `live.p1` | 3 | 18000 | 1828871491 | 3809966672 | 3670598412 | **no** | `plants`, `detritus`, `live.animals_eu`, `live.closed_eu_total`, `live.agent_count`, `live.herbivore_count`, `live.standing_crop`, `live.mean_agent_energy`, `live.births`, `live.deaths`, `live.mean_lifespan` |
| `live.probe` | 1 | 18000 | 3407815438 | 2708624284 | 3112161555 | **no** | — |
| `live.probe` | 2 | 18000 | 4105523088 | 1250855327 | 1214122089 | **no** | — |
| `live.probe` | 3 | 18000 | 808653701 | 4264935648 | 120989536 | **no** | — |
| `reference` | 1 | 18000 | 2399239161 | 2399239161 | 2399239161 | yes | — |
| `reference` | 2 | 18000 | 487902251 | 487902251 | 487902251 | yes | — |
| `reference` | 3 | 18000 | 1244584487 | 1244584487 | 1244584487 | yes | — |
