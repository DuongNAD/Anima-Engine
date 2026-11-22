---
title: Kiến trúc hệ thống — tại sao các ranh giới được đặt như vậy
status: active
owner: architecture
last_reviewed: 2026-11-22
review_cycle: quarterly
---

# Kiến trúc hệ thống — tại sao các ranh giới được đặt như vậy

Tài liệu này giải thích **vì sao** Anima Engine đặt các ranh giới crate, luồng thực thi, lịch chạy ECS, ranh giới IPC, world artifact và runner thí nghiệm như hiện nay trên nhánh thân cây. Đây là tài liệu giải thích (*Explanation* theo Diátaxis), **không** định nghĩa lại hợp đồng hay sao chép số đo:

- Danh mục thư mục và module hiện hành: [`PROJECT.md`](../../PROJECT.md).
- Hợp đồng kiểm thử được (IPC, tất định, sổ cái năng lượng, snapshot, phát triển sinh vật, thí nghiệm): [`docs/reference/README.md`](../reference/README.md).
- Các quyết định kiến trúc bất biến: [`docs/decisions/README.md`](../decisions/README.md).
- Toàn bộ số đo, số lượng test và mức trưởng thành G0–G4 hiện tại: [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền) và [`STATE_OF_THE_PROJECT.md` §2](../planning/STATE_OF_THE_PROJECT.md#2-điều-đã-đúng-và-đúng-ở-mức-nào).

---

## 1. Tổng quan hai runtime và nguyên tắc tách luật khỏi engine

Nếu chỉ nhìn cấu trúc thư mục, dễ tưởng Anima Engine là một ứng dụng Tauri đơn khối bọc lấy Bevy ECS. Thực tế, hệ thống được thiết kế xoay quanh **hai runtime** dùng chung một lõi luật miền (`anima-domain`) và một bộ dựng lịch tick duy nhất (`build_tick_schedule`):

1. **Desktop App Runtime (Tauri + 5 luồng được giám sát):** người dùng tương tác qua React / PixiJS / Three.js, gửi lệnh vào `AppState` (`src-tauri/src/lib.rs`) và nhận snapshot định kỳ từ `SimulationEngine` (`src-tauri/src/core/simulation_loop.rs`).
2. **Headless Experiment Runtime (`anima-lab` + `experiment_runner`):** chạy ensemble nhiều seed, ghép cặp đối chứng và rẽ nhánh nhân quả (`genesis_fork`, `checkpoint_fork`) qua trait `ExperimentModel` (`src-tauri/src/core/experiment_runner.rs`) mà không khởi tạo cửa sổ hay webview.

```mermaid
flowchart TB
    subgraph Domain["Lõi luật độc lập engine: crates/anima-domain"]
        Laws["WorldLawSet / causal / energy / intervention / sim_clock / units"]
    end

    subgraph Desktop["Desktop App Runtime (Tauri)"]
        UI["Frontend React / Pixi / Three.js"]
        IPC["AppState + ObserverSeam + tauri::generate_handler!"]
        Eng["SimulationEngine + ThreadSupervisor (SIM, EMIT, EVO, NET, LEARN)"]
        SimWorld["Bevy World trên luồng SIM"]
        Emit["spawn_emit_thread (30 Hz / ts-rs)"]
        UI <-->|"IPC commands & events"| IPC
        IPC --> Eng
        Eng --> SimWorld
        SimWorld -->|"Arc<RwLock<...>>"| Emit
        Emit -->|"SimulationTickPayload / SimulationStatus"| UI
    end

    subgraph Headless["Headless Experiment Runtime (anima-lab / experiment_runner)"]
        Runner["run_manifest_seed / run_ensemble / run_paired_ensemble / fork"]
        Ref["ReferenceEvolutionWorld (AE1-AE3 + ExoticEnergyField)"]
        Live["LiveExperimentAdapter (RefCell<World> + live_inference_pump_system)"]
        Runner --> Ref
        Runner --> Live
    end

    Sched["build_tick_schedule (lịch tick toàn phần duy nhất)"]
    WG["crates/anima-worldgen (ANMW3-SNSM-v1, tách khỏi anima-engine)"]
    WA["WorldArtifact / WorldIdentity (ANMW v2 đang nối App & Live)"]

    Domain --> Desktop
    Domain --> Headless
    Sched --> SimWorld
    Sched --> Live
    WA --> SimWorld
    WA --> Live
    WG -. "chuẩn bị cho WV2-3 (chưa nối anima-engine)" .-> WA
```

### Tại sao tách ba crate (`anima-domain` · `anima-engine` · `anima-worldgen`)

| Crate | Phụ thuộc được phép | Lý do tồn tại ranh giới |
|---|---|---|
| `anima-domain` (`src-tauri/crates/anima-domain/Cargo.toml`) | Chỉ `serde`, cộng feature opt-in `bevy` (`bevy_ecs`) | Đặt luật bất biến (`causal`, `energy`, `intervention`, `laws`, `sim_clock`, `units`) ở ngoài Bevy/Tauri/Burn. Feature `bevy` cho phép `anima-engine` gắn `#[derive(Resource)]` trực tiếp mà không phải bọc newtype, trong khi mọi mô hình tham chiếu hay kiểm thử thuần miền không phải kéo cả engine. Các module trong `src-tauri/src/core/` re-export lại để call-site cũ không gãy. |
| `anima-engine` (`src-tauri/Cargo.toml`) | Bevy ECS, rapier/physics, tùy chọn feature `desktop`, `networking`, `ml-wgpu` | Giữ build headless/thí nghiệm nhẹ khỏi Neo4j (`neo4rs`), WebSocket (`tokio-tungstenite`), và stack GPU (`burn-wgpu`, `wgpu`). Cổng CI kiểm đồ thị phụ thuộc qua `cargo tree` chứ không chỉ kiểm compile. |
| `anima-worldgen` (`src-tauri/crates/anima-worldgen/Cargo.toml`) | `default = []`; bậc feature `reader` → `generate` → `bake` | Tách bộ sinh địa hình geomorphic World v2 (`ANMW3-SNSM-v1`) khỏi Bevy/Tauri/WGPU để giữ tính bit-exact đa nền tảng (`#![forbid(unsafe_code)]`, `libm`, `ChaCha12Rng`) và không đội chi phí biên dịch lên `anima-engine` trước khi việc nối `WV2-3` sẵn sàng. |

Việc cô lập `anima-domain` là bước tiếp nối sau khi phát hiện logic mô phỏng từng bị hàn chặt vào vỏ Tauri khiến cả fuzzing lẫn Miri đều bị chặn ([`FINDINGS.md`](../../FINDINGS.md#f-007--logic-sim-hàn-chặt-vào-vỏ-tauri-chặn-cả-fuzz-lẫn-miri)).

---

## 2. Luồng mô phỏng chuyên biệt, `ThreadSupervisor` và khởi tạo thế giới

### Tại sao Bevy `World` phải nằm độc quyền trên luồng `SIM`

Bevy `World` yêu cầu quyền truy cập độc quyền `&mut World` ở mỗi tick (`schedule.run(&mut world)`). Nếu để `World` nằm trực tiếp sau một `Mutex<World>` dùng chung với các Tauri async command handler, chỉ cần một lệnh giao diện đọc/sửa trạng thái giữa lúc physics đang bước là luồng UI bị treo hoặc xảy ra deadlock thứ tự khóa.

Vì vậy `SimulationEngine` (`src-tauri/src/core/simulation_loop.rs`) giữ vai trò mặt phẳng điều khiển (*control plane*), giao tiếp với phần còn lại của ứng dụng qua:

- Cờ `running`, các channel có giới hạn dung lượng (`save_request_tx`, `manual_migration_trigger`),
- Các handle chia sẻ an toàn (`SharedLodFocus`, `SharedObserverActions`),
- Ảnh chụp telemetry bảo vệ bởi `Arc<RwLock<...>>` để luồng phát sự kiện đọc mà không chặn bước mô phỏng.

Khi `SimulationEngine::start` chạy, nó khởi tạo 5 luồng hệ điều hành có tên hằng trong `thread_supervisor.rs`: `sup::SIM` (`"sim"`), `sup::EMIT` (`"emit"`), `sup::EVO` (`"evo"`), `sup::NET` (`"net"`), và `sup::LEARN` (`"learn"`). Toàn bộ `World` chỉ được sở hữu bên trong closure của luồng `SIM`.

### Tại sao cần `ThreadSupervisor` và `ExitToken` thay vì `JoinHandle::join()` thuần

`std::thread::JoinHandle::join()` có hai điểm mù nguy hiểm cho cả desktop app lẫn CI: (1) nó chờ vô thời hạn nếu một luồng con kẹt (ví dụ khóa chết trong một channel hoặc thư viện ngoài), và (2) bản thân `join()` không cho biết ngay luồng nào còn đang sống trong khi quá trình tắt đang đếm ngược.

`ThreadSupervisor` (`src-tauri/src/core/thread_supervisor.rs`) giải quyết hai điểm này bằng ba cơ chế nhỏ:

1. Lưu tập tên các luồng đang sống trong `Arc<Mutex<BTreeSet<&'static str>>>` để thứ tự chẩn đoán luôn tất định (và khôi phục mutex nếu bị poison để quá trình tắt không bị mù).
2. `ThreadSupervisor::token` cấp một `ExitToken` kiểu RAII và move thẳng vào bên trong closure của từng luồng.
3. `Drop for ExitToken` gỡ tên luồng khỏi tập `live` cả khi luồng thoát bình thường **lẫn khi panic unwind**. Nhờ vậy `SimulationEngine::stop` gọi `supervisor.wait_for_exit` trong hạn `SHUTDOWN_GRACE_SECS` (30 giây): nếu toàn bộ luồng thoát sớm thì trả về ngay; nếu hết hạn, `stop()` gọi `supervisor.live()` để nêu đích danh luồng kẹt, tách handle thay vì treo vĩnh viễn và vẫn `join` các luồng đã xong để báo cáo panic.

### Tại sao tách `TerrainSource::Generated` khỏi `TerrainSource::AppArtifactOrGenerated`

Khi khởi tạo thế giới qua `init_world` và `init_world_with` (`src-tauri/src/core/ecs.rs`), engine đặt `ResourceField`, `EcosystemBiomass`, `EnergyLedger`, `SeasonClock`, `SimRng`, `BrainPolicy`, `DeterministicMode`, `SpatialHashGrid` và các tài nguyên kiểm kê vào `World`.

Nếu đường thí nghiệm headless dùng chung logic nạp địa hình mặc định của app desktop, nó sẽ vô tình đọc biến môi trường `ANIMA_WORLD_ARTIFACT` hoặc file cache trong `$TMPDIR` (`anima_world_artifact.anmw`) của người dùng trên máy chạy thử, làm kết quả thí nghiệm phụ thuộc vào trạng thái đĩa cục bộ. Enum `TerrainSource` dựng tường lửa ngay tại đầu vào:

- `TerrainSource::AppArtifactOrGenerated` (dùng bởi `init_world` cho desktop app): đọc artifact từ env / `$TMPDIR` / `ANIMA_CACHE_DIR` hoặc sinh mới rồi cache.
- `TerrainSource::Generated` (dùng bởi `LiveExperimentAdapter` và các kiểm thử tất định): luôn sinh địa hình tất định trong bộ nhớ (bằng `OnceLock`), bỏ qua hoàn toàn env và file tạm trên đĩa.

---

## 3. Một Schedule duy nhất, các pha ECS và tính tất định

Quy tắc chuẩn về 5 nguồn rò rỉ phi tất định nằm tại [`DETERMINISM_CONTRACT.md` §2](../reference/DETERMINISM_CONTRACT.md#2-năm-nguồn-rò-rỉ-thế-giới-bên-ngoài). Phần này giải thích **vì sao** cấu trúc lịch tick được thiết kế thành một chuỗi toàn phần duy nhất.

### Tại sao app desktop và `LiveExperimentAdapter` phải gọi cùng `build_tick_schedule`

Trước đây, các bài test headless thường tự ghép một `.chain()` nhỏ cho riêng mình. Hệ quả là đường test và đường app khác nhau về thành phần hệ thống và các điểm flush command (`apply_deferred`), nên một lỗi ở app thật không lộ trên headless hoặc ngược lại. Tệ hơn, khi nhiều hệ thống chỉ nối bằng `.after(...)` cục bộ, bộ lập lịch mặc định của Bevy tự sắp thứ tự tô-pô ở những cặp không có cạnh phụ thuộc — thứ tự đó có thể đổi giữa các tiến trình vì phụ thuộc hash map bên trong (`FINDINGS.md` F-041).

Vì vậy `build_tick_schedule` (`src-tauri/src/core/simulation_schedule.rs`) là **nguồn sự thật duy nhất** cho lịch tick sống, được gọi y hệt ở cả `SimulationEngine::start` lẫn `LiveExperimentAdapter::build`:

1. Tắt `auto_insert_apply_deferred: false` để Bevy không tự chèn điểm sync ngầm giữa các hệ thống.
2. Xâu toàn bộ các hệ thống qua 4 sub-tuple nối tiếp nhau thành **một thứ tự toàn phần duy nhất**, với các điểm `apply_deferred` đặt tường minh tại những ranh giới sinh/xóa thực thể và cập nhật lưới không gian. Tính không mơ hồ của lịch này được canh giữ trực tiếp bởi test `the_tick_schedule_has_no_ambiguous_pair_at_all` (`src-tauri/tests/schedule_ambiguity_tests.rs`).
3. Khi `DeterministicMode` (`src-tauri/src/core/determinism.rs`) bật: `build_tick_schedule` ép executor về `EXECUTOR_SINGLE_THREADED` (`"single-threaded"` qua `executor_name`, thay vì `EXECUTOR_MULTI_THREADED`), chuyển cấp phát định danh sang `RunIdentity` theo bộ đếm (thay `Uuid::new_v4()`), thay đồng hồ tường bằng `tick_timestamp_ms` suy từ tick, và tắt gọi AI ngoài (`allows_external_ai()`).

*Giới hạn trung thực:* đường app desktop mặc định vẫn chạy executor đa luồng trừ khi bật chế độ tất định; hai chế độ dùng cùng thứ tự hệ thống trong `build_tick_schedule`, nhưng chỉ cấu hình tất định mới đóng băng lịch thực thi đơn luồng.

### Tại sao đo pha tick bằng checkpoint xen giữa thay vì tách schedule riêng

Để đo chi phí từng phần của một tick mà không tạo ra một "schedule benchmark" khác với schedule thật, `TickPhase` (`src-tauri/src/core/tick_capture.rs`) chia tick thành 7 biến thể: `FullTick`, `Schedule`, `SensorBrain`, `PhysicsMovement`, `EcologyResources`, `ScheduleTail`, `TelemetryPublish`.

Ba hệ thống đánh dấu ranh giới — `capture_checkpoint_sensor_brain_system`, `capture_checkpoint_physics_system`, `capture_checkpoint_ecology_system` — được đặt cố định ngay trong `build_tick_schedule` với tham số `Option<ResMut<TickCaptureSink>>`. Khi không bật đo, tham số là `None` nên chúng không cấp phát và không chạm bất kỳ trạng thái mô phỏng nào. Đồng thời, `UnavailablePhase` ghi tường minh những pha **không có** trong lịch tick sống (như `plant_soil_weather` của `core::dynamic_fields`) để người đọc dữ liệu hiệu năng không bao giờ nhầm "vắng mặt" thành "chi phí 0 ms".

---

## 4. Ranh giới IPC, `ObserverSeam` và hợp đồng Rust ↔ TypeScript

Bảng danh sách các command và event hợp lệ nằm tại [`IPC_CONTRACT.md`](../reference/IPC_CONTRACT.md). Kiến trúc ở ranh giới này tồn tại để khóa ba lỗ hổng âm thầm: rò rỉ người quan sát, churn bộ nhớ ở 30 Hz, và lệch kiểu giữa Rust với TypeScript.

### `ObserverPolicy` và `ObserverSeam`: đóng nguồn rò rỉ thứ năm

Theo [ADR-0004](../decisions/ADR-0004-observer-as-declared-intervention.md) và [`DETERMINISM_CONTRACT.md` §2.1](../reference/DETERMINISM_CONTRACT.md#21-nguồn-thứ-năm-camera-adr-0004), nếu vị trí camera (`LodFocus`) tự động hạ tầng suy nghĩ của agent ở xa xuống `Cold` để tiết kiệm CPU, thì chính hành động "nhìn vào góc bản đồ khác" của người quan sát đã làm thay đổi quỹ đạo tiến hóa của thế giới — trong khi ở bên ngoài trông như chỉ là render.

Để chặn rò rỉ này:

- `ObserverPolicy` (`src-tauri/src/core/observer.rs`) tách ba chế độ tường minh: `Absent` (không có người quan sát), `Spectate` (quan sát nhưng **cấm** mọi thay đổi tiering/physics, được kiểm chứng trực tiếp bởi test `spectate_matches_absent`), và `Inhabit { cause_id }` (cho phép `LodFocus` hạ tầng `Cold` — hành vi được kiểm bởi `cold_agents_stop_asking_entirely` — nhưng sự can thiệp được ghi nhận thành nhân quả).
- Các lệnh Tauri đi qua `AppState` và `tauri::generate_handler!` (`src-tauri/src/lib.rs`). Những đột biến trạng thái có ảnh hưởng tới cấu hình tiến hóa hay di cư (`ObserverAction`) phải đi qua `ObserverSeam`: một wrapper mức A ghép thao tác ghi log vào `SharedObserverActions` (hàng đợi giới hạn 64 phần tử) **ngay trước** khi đổi trạng thái dùng chung.
- Trong lịch tick, `drain_observer_actions_system` rút hàng đợi này không cấp phát heap, còn `record_observer_trace_system` chạy ngay sau đồng bộ `LodFocus` để ghi lại đúng tầm nhìn đã thực sự áp lên thế giới ở tick đó.

### Tại sao `emit.rs` tách nhịp 30 Hz, tái sử dụng buffer và vẫn xả event khi headless

Luồng mô phỏng `SIM` có thể chạy nhanh (mục tiêu 60 Hz hoặc chạy tốc độ tối đa), trong khi gửi JSON qua IPC vào WebView ở mỗi tick sẽ làm nghẽn serialization và DOM. Vì vậy `spawn_emit_thread` (`src-tauri/src/core/emit.rs`) đọc các snapshot từ `EmitChannels` theo nhịp `EMIT_INTERVAL` (`33 ms`, ~30 Hz) và giới hạn `pheromone-update` ở `PHEROMONE_EMIT_INTERVAL` (`100 ms`):

- `new_tick_payload` cấp phát sẵn `SimulationTickPayload` theo dung lượng dự kiến; ở các khung hình tiếp theo, `refresh_tick_payload` chỉ `clear()` và `extend_from_slice()` trên cùng vector/map để đạt 0 cấp phát heap ở trạng thái ổn định.
- Ngay cả khi `app_handle` là `None` (khi `SimulationEngine` chạy trong bài test headless không có cửa sổ Tauri), luồng emit vẫn phải xả hàng đợi `combat_events` nếu không vector sự kiện sẽ phình vô hạn.

### Tại sao dùng `ts-rs` và snapshot khoa học tách khỏi file save thường

Thay vì viết tay các interface TypeScript rồi mong hai bên khớp nhau (nguyên nhân từng gây lỗi `head_directions` lệch kiểu map vs array), các struct qua ranh giới IPC — cụ thể là `SimulationTickPayload` và `SimulationStatus` (`src-tauri/src/core/simulation_state.rs`) — gắn `#[derive(ts_rs::TS)]` để `cargo test --lib export_bindings` sinh thẳng 51 file `.ts` vào `src/types/generated/`. Cổng CI chạy `git diff --exit-code` trên thư mục sinh này để mọi thay đổi struct Rust không tái sinh type đều thất bại ngay.

Tương tự, `SavedSimulationState` được bọc trong `SnapshotEnvelope` (`src-tauri/src/core/snapshot.rs`) với SHA-256, `float_roundtrip` và ghi nguyên tử thay vì dùng định dạng save thường dễ mất bit hoặc hỏng dở khi mất điện (chi tiết hợp đồng tại [`SNAPSHOT_CONTRACT.md`](../reference/SNAPSHOT_CONTRACT.md)).

---

## 5. World Artifact và việc tách crate `anima-worldgen`

Vì sao thế giới phải được đóng gói thành một artifact duy nhất thay vì để frontend và backend tự sinh theo cùng một seed? Ngay cả với cùng seed, một hàm noise viết bằng JS/WebGL trên frontend và một hàm viết bằng Rust trên backend có thể sai khác ở phép làm tròn float, dẫn tới cảnh renderer đặt cây hoặc mặt nước ở chỗ backend coi là vách đá ([`WORLD_DESIGN.md` §4](../../WORLD_DESIGN.md#4-kiến-trúc-mục-tiêu-một-world-artifact-sinh-1-lần-cache-dùng-chung); hợp đồng tọa độ và biome nằm tại [`COORDINATE_CONTRACT.md`](../../COORDINATE_CONTRACT.md), [`BIOME_TAXONOMY.md`](../../BIOME_TAXONOMY.md) và [`MAP_MANIFEST.md`](../../MAP_MANIFEST.md)).

### ANMW v2 hiện hành (`world_artifact.rs`)

Trên nhánh thân cây hiện tại, cầu nối thực tế giữa frontend và backend là định dạng nhị phân phẳng little-endian ANMW v2 (`src-tauri/src/core/world_artifact.rs`):

- `WorldArtifact` (`WORLD_ARTIFACT_MAGIC = b"ANMW"`, `WORLD_ARTIFACT_VERSION = 2`, `CANONICAL_WORLD_SCALE = 200.0`) chứa lưới độ cao, độ ẩm, nhiệt độ, dòng chảy và mảng 22 biome. `WorldArtifact::peek_seed` đọc nhanh seed ở header mà chưa cần giải mã toàn payload; `WorldArtifact::from_bytes` kiểm ma thuật, phiên bản, tính hữu hạn của float và checksum `fnv1a_32` thuần không phụ thuộc ngoài (khớp byte-for-byte với hàm TS phía frontend).
- `WorldIdentity` (`WorldIdentity::from_terrain`, `WorldIdentity::mismatch_against`) gắn dấu vân tay (`seed`, `generator_version`, `terrain_checksum`, kích thước) làm ECS `Resource` và lưu cùng file save. Nhờ đó một file save của thế giới A không thể âm thầm nạp lên địa hình của thế giới B mà không bị phát hiện.

### Tại sao tách `anima-worldgen` (World v2 / `ANMW3-SNSM-v1`) thành crate riêng

Để nâng cấp lên thế giới ô lớn có kiến tạo địa chất, thủy văn và xói mòn thật ([ADR-0010](../decisions/ADR-0010-pinned-world-artifact.md)), dự án tách `src-tauri/crates/anima-worldgen/` gồm các module `config` (`schema_tag` `"ANMW3-SNSM-v1"`), `tectonic`, `hydrology`, `erosion_fluvial`, `erosion_thermal`, `rng`, cùng hằng chuẩn hóa `WORLD_EXTENT_M` / `normalize_elevation`:

- **Bit-exact đa HĐH:** bật `#![forbid(unsafe_code)]`, `#![deny(clippy::disallowed_methods)]` và file `clippy.toml` cấm các hàm siêu việt của `std` (`f32::sin`, `f64::powf`, ...) để ép mọi phép tính đi qua `libm` và `ChaCha12Rng` theo từng giai đoạn/từng ô.
- **Cô lập biên dịch:** mặc định `default = []`, phân tầng feature `reader` → `generate` → `bake`, và được CI chặn không cho `bevy_ecs`, `tauri`, `wgpu` lọt vào `anima-worldgen`.

*Trạng thái mã trên thân cây:* `anima-worldgen` hiện **chưa** được nối vào `anima-engine` (công việc dự kiến ở `WV2-3`; CI chủ động kiểm `anima-worldgen` vắng mặt trong cây phụ thuộc mặc định của `anima-engine`). Vì vậy ANMW v2 (`world_artifact.rs`) vẫn là định dạng cầu nối đang chạy ở desktop app và `LiveExperimentAdapter`.

---

## 6. `anima-domain`, Experiment Runner và hai Adapter mô phỏng

Hợp đồng chi tiết cho thí nghiệm nằm ở [`EVOLUTION_EXPERIMENT_CONTRACT.md`](../reference/EVOLUTION_EXPERIMENT_CONTRACT.md), sổ cái năng lượng ở [`ENERGY_LEDGER_CONTRACT.md`](../reference/ENERGY_LEDGER_CONTRACT.md) và [`SIMULATION_RULES.md`](../../SIMULATION_RULES.md), còn nền tảng thiết kế của năng lượng ngoại lai nằm ở [ADR-0002](../decisions/ADR-0002-world-laws-and-exotic-energy.md).

### Tại sao tách `ExperimentModel` khỏi `ExperimentManifest` và runner

Mỗi thí nghiệm khoa học trong Anima gồm ba phần có vòng đời khác nhau:

1. **Đặc tả đầu vào bất biến:** `ExperimentManifest` (`src-tauri/src/core/experiment/manifest.rs`) gói danh tính thế giới, `WorldLawSet` (gồm `BaselineEnergyLaw` và tùy chọn `exotic_energy`), `InitialConditionSet`, lịch can thiệp, danh sách seed và tần suất lấy mẫu thành một dấu vân tay chuẩn hóa. Khi chạy đối chứng, `FactorDiff` kiểm tra chứng minh hai manifest control/treatment **chỉ** khác nhau đúng ở danh sách đường dẫn nhân tố được khai báo, còn mọi sai phạm cấu hình trả về `ExperimentError`.
2. **Danh mục quan sát & can thiệp có kiểm kiểu:** `ObservableRegistry` (`reference_default` và `live_default` trong `src-tauri/src/core/experiment/observables.rs`) khóa đơn vị và phép tổng hợp của từng biến quan sát. Các can thiệp (`InterventionKind`, `InterventionCommand`, `InterventionQueue` trong `anima-domain`) được kiểm hợp lệ trước khi chạy; trên thế giới Bevy sống, `apply_live_interventions_system` áp chúng đúng tick định sẵn và ghi nhân quả vào `CausalLedger`.
3. **Bộ điều phối thống kê & bảo toàn thất bại (`experiment_runner`):** thông qua trait `ExperimentModel`, các hàm `run_manifest_seed`, `run_ensemble`, `run_paired_ensemble`, `run_paired_ensemble_with_control` thực thi từng seed dưới lớp cách ly `catch_unwind` và trả về `RunResult` cùng `RunStatus` (`Completed` hoặc `Failed`) — **không** bao giờ âm thầm bỏ qua hay điền số 0 cho seed lỗi. `genesis_fork`, `checkpoint_fork` và `checkpoint_fork_with_exotic` tạo các nhánh đối chứng từ `t=0` hoặc từ snapshot giữa chừng, xuất `EnsembleSummary` / `PairedEnsembleReport`, rồi tính suy luận bắt cặp (`paired_effect`) tách biệt rõ với thống kê mô tả không ghép cặp (`compare_ensembles`).

*(Lưu ý phân biệt hai struct cùng tên: `core::experiment_runner::RunProvenance` sinh mã định danh nguồn gốc tất định cho mỗi lần chạy thí nghiệm headless, còn `core::telemetry_export::RunProvenance` là struct xuất `ts-rs` cho bộ dữ liệu telemetry của desktop app.)*

### Tại sao có tới hai Adapter (`ReferenceEvolutionWorld` vs `LiveExperimentAdapter`)

| Tiêu chí | `ReferenceEvolutionWorld` (`src-tauri/src/core/reference_world.rs`) | `LiveExperimentAdapter` (`src-tauri/src/core/live_experiment.rs`) |
|---|---|---|
| Phiên bản model | `MODEL_VERSION` (`"reference-evolution-world/1"`) | `LIVE_MODEL_VERSION` (`"live-bevy-world/2"`) |
| Mục đích | Mô hình tham chiếu tổng hợp chạy rất nhanh, hiện thực lát headless AE1–AE3 | Bọc `RefCell<World>` + `build_tick_schedule` + `TerrainSource::Generated` để thí nghiệm trên chính lịch Bevy ECS thật |
| Năng lượng ngoại lai (`MU`) | Hỗ trợ `ExoticEnergyField` và quần thể tham chiếu `ReferencePopulation` | **Chủ động từ chối** manifest có `laws.exotic_energy` vì lịch tick sống chưa có trường MU |
| Suy luận não agent | Không chạy não không gian từng cá thể | Dùng `live_inference_pump_system` chạy đồng bộ trên `BrainModel` đóng băng với buffer cấp phát sẵn (yêu cầu tick *n* trả kết quả tất định ở tick *n+1*) |
| Tần suất & drift | Chạy theo bước ecology gộp | Khai báo 29 khóa `LIVE_KEYS`, 12 biến `LIVE_OBSERVABLE_IDS` (cộng `LIVE_EXTENDED_OBSERVABLE_IDS`), tách `live_observables` (tiêu thụ cửa sổ) với `live_observables_peek`, và hỗ trợ so sánh song hành `run_live_drift_pair` / `run_live_drift_ensemble` qua `TurnoverSchedule` |

Cả hai adapter đều được điều phối thống nhất từ CLI `anima-lab` (`src-tauri/src/bin/anima-lab.rs`) qua enum `Model::Reference` hoặc `Model::Live`.

Việc tách hai adapter giúp tránh một ngộ nhận chết người: dùng mô hình ODE/cohort 7 biến để chạy nhanh 10.000 bước không có nghĩa nó thay thế cho tương tác không gian–vật lý 256×256 của ECS thật, và ngược lại. Mối liên hệ giữa chúng là **chia sẻ cùng luật bất biến từ `anima-domain` và cùng giao thức `ExperimentModel`**, chứ không phải tương đương số học.

*Giới hạn trung thực:* theo nguyên tắc giữ nguyên biên bằng chứng của chính sách tài liệu, việc `LiveExperimentAdapter` đã kiểm được đường chạy headless (bao gồm các chế độ opt-in như [ADR-0005](../decisions/ADR-0005-live-population-turnover.md), [ADR-0006](../decisions/ADR-0006-locomotion-actuation.md), [ADR-0007](../decisions/ADR-0007-live-ecology-sustainability.md) và khung thời gian [ADR-0008](../decisions/ADR-0008-world-past-and-time.md)) **không** tự động suy ra mọi giả thuyết tiến hóa trên app desktop đều đã sẵn sàng hay đã có World Lab UI; hiện trạng được chứng minh từng dòng ở [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).
