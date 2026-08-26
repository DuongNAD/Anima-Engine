---
title: Hợp đồng prompt sinh vật (genotype → prompt → asset 3D)
status: proposed
owner: simulation-architecture
last_reviewed: 2026-08-25
review_cycle: per-release
design: ../ai/design/2026-08-25-feature-genome-to-mesh.md
testing: ../ai/testing/2026-08-25-feature-living-world.md
---

# Hợp đồng prompt sinh vật

Tài liệu này là nguồn chuẩn bắt buộc cho mọi thay đổi liên quan tới việc dịch một sinh vật
đã phát triển thành **prompt text-to-3D** và tiêu thụ **asset** trả về. Nếu code, kế hoạch
hay comment khác tài liệu này, sửa nguồn mâu thuẫn trước khi triển khai.

Đọc kèm: [`CREATURE_DEVELOPMENT_CONTRACT.md`](./CREATURE_DEVELOPMENT_CONTRACT.md) (nguồn của
`DevelopedPhenotype`) và [`DETERMINISM_CONTRACT.md`](./DETERMINISM_CONTRACT.md) (§5, cách một
model từ xa được phép tham gia một run tái lập được).

---

## 1. Lời hứa, và giới hạn của nó

> **Cùng phenotype ⇒ cùng prompt, byte-exact. Cùng prompt + cùng seed + cùng model version
> ⇒ cùng asset, vì asset được lấy từ kho, không lấy từ mạng.**

Vế đầu là một lời hứa mạnh và **kiểm được bằng test thuần**: prompt là hàm thuần của
`MorphoSpec`. Vế sau là một lời hứa yếu hơn về bản chất — không nhà cung cấp nào bảo đảm
bit-identical qua thời gian — nên nó được giữ bằng **kho asset**, không bằng niềm tin vào API:
lần sinh đầu tiên gọi mạng, mọi lần sau đọc đĩa.

Điều tài liệu này **không** hứa: prompt không bảo đảm mesh trả về có đúng số chi. Không có
prompt nào làm được điều đó. Độ đúng đặc điểm đến từ **prompt + cổng nghiệm thu hình học +
retry theo seed + kho asset đã duyệt** (§9), chứ không từ riêng câu chữ.

---

## 2. Thuật ngữ

| Thuật ngữ | Ý nghĩa |
|---|---|
| `MorphoSpec` | IR số hoá, hàm thuần của `DevelopedPhenotype` + `EnvSample` đã lưu |
| Slot | Một mệnh đề trong prompt, có thứ tự cố định và mức ưu tiên |
| Lexicon | Bảng ánh xạ band số → cụm từ tiếng Anh; là dữ liệu, versioned |
| Style lock | Mệnh đề nghệ thuật **cố định**, giống hệt trong mọi prompt của một run |
| `ArchetypeKey` | `blake3(prompt ‖ negative ‖ params ‖ model_version ‖ seed)` — khoá kho asset |
| Archetype | Tập cá thể cùng `ArchetypeKey`; dùng chung **một** mesh |
| Asset | File `.glb` đã nghiệm thu, nằm trong kho, địa chỉ hoá bằng `ArchetypeKey` |
| Per-individual fit | Biến đổi rẻ, tất định, áp asset lên số đo thật của từng cá thể (§8) |

---

## 3. Invariant bắt buộc

### P01 — Prompt là hàm thuần

`build_prompt(spec: &MorphoSpec, lexicon: &Lexicon) -> Prompt` không đọc đồng hồ, không đọc
RNG, không đọc môi trường hiện tại của cá thể, không đọc mạng. Cùng input ⇒ cùng `String`.

### P02 — Một prompt ⇔ một asset

`ArchetypeKey` băm **chính chuỗi prompt cuối cùng** cùng negative prompt, tham số sinh, phiên
bản model và seed. Không được băm một tập band "gần đúng" rồi sinh prompt khác — làm vậy thì
hai prompt khác nhau cùng trỏ một asset và không ai biết mesh trong kho được sinh bằng câu nào.

### P03 — Tham số trước, câu chữ sau

Bất kỳ thuộc tính nào nhà cung cấp phơi ra dưới dạng **tham số** (`seed`, `art_style`,
`topology`, `target_polycount`, `symmetry`, `negative_prompt`) **MUST** đi vào tham số và
**MUST NOT** lặp lại trong chuỗi prompt. Ký tự trong prompt là tài nguyên khan hiếm; đừng tiêu
nó vào thứ API đã có ô riêng, và tham số thì được model tuân thủ chặt hơn prose.

### P04 — Ngân sách ký tự

Prompt cuối cùng **MUST** ≤ **1000 ký tự** (hard cap), và **SHOULD** ≤ **700**. Đây không phải
mục tiêu cần lấp đầy: các model text-to-3D suy giảm chú ý rõ rệt ở phần đuôi, nên một prompt
420 ký tự đặt đúng thứ tự cho kết quả tốt hơn một prompt 950 ký tự nhồi thêm tính từ. Khi vượt
ngân sách, **cắt theo bảng ưu tiên §5**, không cắt cụt giữa mệnh đề.

### P05 — Style lock bất biến trong một run

Mệnh đề nghệ thuật (§6.2) **MUST** giống hệt nhau cho mọi sinh vật trong cùng một run và được
ghi vào manifest. Đây là điều tách một menagerie trông như **một thế giới** khỏi một bộ sưu
tập clip-art: mỗi con sinh độc lập với art direction trôi nổi thì tổng thể sẽ lệch pha, dù từng
con đẹp.

### P06 — Asset không bao giờ là nguồn sự thật hình học

Physics, `SpatialCollider`, MAP-Elites descriptor, save file và mọi phép đo đều đọc
`DevelopedPhenotype`. Asset chỉ được **hiển thị**. Đây không phải hạn chế: chính nó cho phép
một mesh phục vụ hàng nghìn cá thể có số đo khác nhau (§8), và đó là lý do chi phí API rơi từ
hàng nghìn đô xuống hàng chục.

### P07 — Thiếu asset là trạng thái hợp lệ

Kho rỗng, mạng hỏng, quota hết, nghiệm thu trượt — tất cả đều dẫn tới **mesh procedural**, không
dẫn tới lỗi hay chờ. Không có `await` nào nằm giữa "quyết định spawn" và "sinh vật xuất hiện".

### P08 — Asset không tham gia hash canonical

Checksum của một run, share string và mọi gate tất định **MUST** giữ nguyên giá trị khi thư mục
asset xuất hiện hoặc biến mất. Kho asset có hash riêng của nó, công bố riêng.

### P09 — Trần chi phí là cấu hình, không phải hy vọng

`max_archetypes_per_run` (mặc định **200**) và `max_requests_per_hour` là hằng số có tên. Khi
chạm trần, archetype mới dùng mesh procedural và ghi một dòng lý do. Không có đường nào để một
run tự ý gọi API 10⁵ lần.

### P10 — Lexicon là dữ liệu versioned

`Lexicon` có `version: u16`. Đổi một cụm từ là đổi version, và đổi version là **làm mới toàn bộ
`ArchetypeKey`** — asset cũ vẫn dùng được cho phenotype cũ, asset mới sinh cho phenotype mới.
Không sửa lexicon tại chỗ mà giữ nguyên key.

---

## 4. `MorphoSpec` — đầu vào của prompt

Hàm thuần của `DevelopedPhenotype` + `EnvironmentSnapshot` đã lưu trong phenotype đó (D02).
Không đọc môi trường **hiện tại** — nếu đọc, con vật sẽ đổi ngoại hình khi đi bộ, đúng cái bẫy
mà [D08](./CREATURE_DEVELOPMENT_CONTRACT.md) cấm.

```rust
pub struct MorphoSpec {
    pub version: u16,
    pub body_plan: BodyPlan,        // suy từ topology graph, §4.1
    pub size_class: SizeClass,      // từ total_mass, log-band
    pub build: Build,               // từ aspect ratio
    pub limb: LimbProfile,          // count + length band + terminal
    pub covering: Covering,         // temp × moisture × medium
    pub color: ColorTerm,           // display_color → HSL → tên
    pub pattern: Option<Pattern>,   // biome, chỉ khi predation cao
    pub head: HeadProfile,          // AgentType + ánh sáng
    pub thermo: Option<ThermoTerm>, // Allen, chỉ ở cực nhiệt
    pub distinctive: Option<Distinctive>, // tối đa MỘT
}
```

### 4.1 Suy `BodyPlan` từ graph

`MorphologyGenotype` là graph capsule, không có nhãn "quadruped". Phân loại tất định:

1. Tìm **axial chain**: đường dài nhất từ root theo tổng `length`.
2. **Limb chain** = mọi nhánh rời khỏi axial chain.
3. Phân loại:

| Điều kiện | `BodyPlan` |
|---|---|
| `limb_chains == 0` và `axial_len / mean_radius > 6` | `Serpentine` |
| `limb_chains == 0` còn lại | `Blob` |
| `medium == Aquatic` và `limb_chains ∈ 2..=6` và `limb_len_ratio < 0.35` | `Finned` |
| `medium == Aerial` | `Winged` |
| `limb_chains == 2` | `Biped` |
| `limb_chains ∈ 4..=6` | `Quadruped` |
| còn lại | `Radial` |

Ngưỡng nằm trong `Lexicon`, không hardcode rải rác. Đổi ngưỡng ⇒ đổi `Lexicon.version` (P10).

---

## 5. Ngữ pháp prompt

Thứ tự slot **cố định**. Nối bằng `", "`. Không dấu chấm cuối (một số backend coi dấu câu là
token lãng phí).

| # | Slot | Bắt buộc | Ưu tiên cắt | Ngân sách gợi ý |
|---|---|---|---|---|
| S1 | Subject: size + build + body plan | ✅ | không bao giờ cắt | 45 |
| S2 | Proportions | ✅ | không bao giờ cắt | 35 |
| S3 | Limbs: count + length + terminal | khi có chi | 5 (cắt sau cùng trong nhóm tuỳ chọn) | 60 |
| S4 | Surface: covering + color | ✅ | không bao giờ cắt | 45 |
| S5 | Pattern | ⬜ | 3 | 40 |
| S6 | Head & senses | ✅ | không bao giờ cắt | 75 |
| S7 | Thermoregulation (Allen) | ⬜ | 2 | 45 |
| S8 | Distinctive feature (tối đa 1) | ⬜ | **1 — cắt đầu tiên** | 35 |
| S9 | Style lock | ✅ | không bao giờ cắt | 155 |

Tổng khi đủ slot ≈ **535 ký tự**, còn dư lớn dưới trần 1000. Ngân sách rộng là cố ý: nó cho
lexicon chỗ để mô tả *cụ thể hơn* ở các slot đã có, thay vì cho phép thêm slot mới.

### 5.1 Lexicon — S1 Subject

**Size class** (từ `total_mass`, kg, log-band):

| Band | kg | Từ |
|---|---|---|
| `Tiny` | < 0.1 | `tiny` |
| `Small` | 0.1 – 5 | `small` |
| `Medium` | 5 – 50 | `medium-sized` |
| `Large` | 50 – 300 | `large` |
| `Huge` | > 300 | `massive` |

> **Không dùng neo động vật thật** (`cat-sized`, `bear-like`). Chúng truyền cỡ tốt nhưng kéo
> theo **toàn bộ body plan** của loài được nhắc, và đó là chế độ hỏng phổ biến nhất của
> text-to-3D: yêu cầu một con sáu chân, nhắc "dog-sized", nhận về một con chó.

**Build** (từ `axial_len / mean_radius`):

| Band | Tỉ lệ | Từ |
|---|---|---|
| `Slender` | > 5.0 | `slender` |
| `Balanced` | 2.5 – 5.0 | *(bỏ trống — tiết kiệm ký tự)* |
| `Stocky` | < 2.5 | `stocky` |

**Body plan noun**:

| `BodyPlan` | Cụm từ |
|---|---|
| `Serpentine` | `serpentine creature` |
| `Quadruped` | `four-legged creature` |
| `Biped` | `two-legged creature` |
| `Finned` | `finned aquatic creature` |
| `Winged` | `winged creature` |
| `Radial` | `radially symmetric creature` |
| `Blob` | `limbless soft-bodied creature` |

S1 = `a {size} {build} {body_plan_noun}` (bỏ khoảng trắng thừa khi `build` rỗng).

### 5.2 Lexicon — S2 Proportions

| Điều kiện | Cụm từ |
|---|---|
| `Serpentine` | `long tapering body` |
| `Slender` + `Quadruped`/`Biped` | `long low body` |
| `Stocky` + bất kỳ | `deep barrel torso` |
| `Finned` | `torpedo-shaped body` |
| `Winged` | `light narrow torso` |
| còn lại | `compact rounded body` |

### 5.3 Lexicon — S3 Limbs

`{count_word} {length_word} {terminal}`

- **count_word**: `two` / `four` / `six` / `eight` (>8 ⇒ `many`)
- **length_word** (từ `limb_len / axial_len`):

| Tỉ lệ | Từ |
|---|---|
| < 0.15 | `stubby` |
| 0.15 – 0.30 | `short` |
| 0.30 – 0.50 | `medium-length` |
| 0.50 – 0.75 | `long` |
| > 0.75 | `stilt-like` |

- **terminal** (từ `LocomotionMedium` × `AgentType`):

| Medium | AgentType | Cụm từ |
|---|---|---|
| `Terrestrial` | `Predator` | `limbs ending in clawed paws` |
| `Terrestrial` | `Prey` | `limbs ending in blunt hooved feet` |
| `Amphibious` | bất kỳ | `limbs ending in broad webbed feet` |
| `Aquatic` | bất kỳ | `webbed paddle fins` |
| `Aerial` | bất kỳ | `membranous wings` |
| `Arboreal` | bất kỳ | `limbs ending in long grasping digits` |

### 5.4 Lexicon — S4 Surface

**Covering** — `field_temperature` và `moisture` đều chuẩn hoá `[0,1]`
(xem `TerrainMap.temperatures`, `TerrainMap.moistures`):

| Temp | Moisture | Medium | Cụm từ |
|---|---|---|---|
| < 0.25 | bất kỳ | `Terrestrial` | `thick shaggy fur` |
| < 0.25 | bất kỳ | `Aquatic`/`Amphibious` | `dense oily pelt` |
| 0.25 – 0.45 | bất kỳ | `Terrestrial` | `short dense fur` |
| 0.45 – 0.70 | < 0.4 | `Terrestrial` | `smooth tough hide` |
| 0.45 – 0.70 | ≥ 0.4 | `Terrestrial` | `sleek short fur` |
| > 0.70 | < 0.4 | `Terrestrial` | `dry keratinous scales` |
| > 0.70 | ≥ 0.4 | `Terrestrial` | `glossy moist skin` |
| bất kỳ | bất kỳ | `Aquatic` | `smooth slick skin` |
| bất kỳ | bất kỳ | `Aerial` | `layered feathers` |

**Color** — `display_color` RGB → HSL, lượng tử hoá tất định:

- `saturation < 0.15` ⇒ vô sắc theo `lightness`: `charcoal` (<0.33), `ash-grey` (<0.66), `bone-white`.
- ngược lại, hue chia **12 cung 30°**:

| Cung | Tên | Cung | Tên |
|---|---|---|---|
| 0 | `rust-red` | 6 | `jade` |
| 1 | `burnt-orange` | 7 | `teal` |
| 2 | `ochre` | 8 | `slate-blue` |
| 3 | `straw-yellow` | 9 | `indigo` |
| 4 | `olive` | 10 | `violet` |
| 5 | `moss-green` | 11 | `mulberry` |

- Tiền tố độ sáng: `dark ` (<0.33) / *(rỗng)* / `pale ` (>0.66).
- Tiền tố độ bão hoà: `muted ` khi `saturation < 0.45`, ngược lại rỗng.
- **Nhánh vô sắc không nhận tiền tố nào.** `charcoal`/`ash-grey`/`bone-white` đã mã hoá độ sáng;
  `pale bone-white` là hai lần cùng một thông tin và tiêu ký tự vô ích.

S4 = `{covering}, {sat_prefix}{light_prefix}{hue_name}` — ví dụ `thick shaggy fur, muted dark olive`.

### 5.5 Lexicon — S5 Pattern

Chỉ phát khi `AgentType == Prey` **hoặc** predation pressure ở band cao. Nguồn là
`EnvironmentSnapshot.biome` (11-biome projection, `BiomeType` tại `core/terrain.rs:114`):

| `BiomeType` | Cụm từ |
|---|---|
| `TemperateForest`, `BorealForest`, `Rainforest` | `dappled bark-toned markings` |
| `Grassland` | `faint tawny striping` |
| `Desert`, `Beach` | `sand-toned countershading` |
| `Snow`, `MountainRock` | `pale mottled winter coat` |
| `Ocean`, `DeepOcean`, `River` | `silver countershaded flanks` |

### 5.6 Lexicon — S6 Head & senses

| `AgentType` | Cụm từ |
|---|---|
| `Predator` | `broad head with forward-facing eyes and short powerful jaws` |
| `Prey` | `narrow head with wide-set lateral eyes and a blunt grazing muzzle` |

Bổ ngữ ánh sáng thấp (`DeepOcean`, hoặc elevation dưới ngưỡng hang, hoặc trait ăn đêm) thay
cụm mắt bằng `oversized reflective eyes`.

### 5.7 Lexicon — S7 Thermoregulation

Quy tắc Allen, chỉ ở hai cực — vùng ôn hoà bỏ trống để tiết kiệm ngân sách:

| Temp | Cụm từ |
|---|---|
| < 0.25 | `small rounded ears and a short thick tail` |
| > 0.75 | `large radiating ears and a long thin tail` |

### 5.8 Lexicon — S8 Distinctive

**Tối đa một**, chọn theo độ lệch chuẩn hoá lớn nhất so với trung vị quần thể. Nhiều hơn một
đặc điểm nổi bật thì model sẽ trộn chúng thành thứ không đọc được.

| Nguồn | Cụm từ |
|---|---|
| Armor trait cao | `bony dorsal plates` |
| Spine trait cao | `a ridge of defensive spines` |
| `ExoticEnergy` biểu hiện (ADR-0002) | `faint bioluminescent markings` |
| Tốc độ cao | `deep chest and long hind limbs` |
| Đào hang | `broad shovel-like forelimbs` |

---

## 6. Phần cố định

### 6.1 Negative prompt (tham số, P03)

```text
human, humanoid, anthropomorphic, clothing, armor, weapon, saddle, rider,
text, watermark, logo, multiple creatures, herd, scenery, terrain, ground plane,
base, pedestal, cropped, extra limbs, fused limbs, duplicated head
```

`human, humanoid, anthropomorphic` đứng đầu là có lý do: dữ liệu huấn luyện của mọi model
text-to-3D lệch nặng về nhân hình, và một sinh vật hai chân không có negative prompt sẽ ra
người trong phần lớn số lần.

### 6.2 Style lock (trong prompt, P05)

```text
stylized realistic creature design, symmetrical anatomy, neutral A-pose,
single full-body creature, plain empty background, even matte PBR texture
```

Không viết `quad topology`, `low poly`, `4k texture`, `game-ready` vào đây nếu nhà cung cấp có
tham số tương ứng — P03. Chuỗi trên chỉ giữ phần **không** ánh xạ được sang tham số.

### 6.3 Tham số sinh

| Tham số | Giá trị | Ghi chú |
|---|---|---|
| `seed` | từ `WorldIdentity.seed` + `ArchetypeKey` | phải khai báo; đừng để nhà cung cấp tự chọn |
| `model_version` | **pin tường minh** | thả nổi là mất tái lập ở lần sinh mới |
| `art_style` | một giá trị duy nhất cho cả run | P05 |
| `topology` | quad nếu có | cần cho skinning |
| `target_polycount` | ngân sách đã công bố | |
| `symmetry` | bật nếu có | |

Bảng năng lực thực tế của từng nhà cung cấp **chưa đo**; điền ở Slice 1 sau khi thử, đừng chép
từ tài liệu marketing.

---

## 7. Thuật toán lắp ráp

```rust
pub fn build_prompt(spec: &MorphoSpec, lex: &Lexicon) -> Prompt {
    let mut slots: Vec<(Priority, String)> = Vec::with_capacity(9);
    // đẩy S1..S9 theo đúng thứ tự, bỏ qua slot None
    let mut text = join(&slots, ", ");
    // cắt theo ưu tiên tăng dần cho tới khi đạt ngân sách
    while text.chars().count() > lex.hard_cap {
        drop_lowest_priority_optional(&mut slots);  // panic nếu chỉ còn slot bắt buộc
        text = join(&slots, ", ");
    }
    Prompt { text, negative: lex.negative.clone(), params: lex.params.clone() }
}
```

Ba điều bắt buộc trong hiện thực:

1. Đếm bằng `chars().count()`, không phải `len()` — ngân sách là ký tự, không phải byte.
2. Cắt ở ranh giới slot. Không bao giờ cắt cụt giữa một mệnh đề: một mệnh đề cụt tệ hơn một
   mệnh đề vắng, vì nó vẫn tiêu chú ý của model mà không truyền được đặc điểm nào.
3. Nếu đã cắt hết slot tuỳ chọn mà vẫn vượt trần thì đó là lỗi lexicon, không phải lỗi runtime
   — fail to lên, đừng lặng lẽ cắt cụt.

---

## 8. Archetype và per-individual fit

Đây là cơ chế làm cho ý tưởng chạy được ở quy mô quần thể.

Nhiều cá thể cho **cùng** một prompt — vì lexicon lượng tử hoá theo band — nên chúng dùng
**chung một asset**. Khác biệt giữa các cá thể trong cùng archetype được áp bằng biến đổi rẻ,
tất định, chạy in-process:

| Biến thể | Cách áp | Nguồn số |
|---|---|---|
| Kích cỡ tổng | uniform scale | `DevelopedPhenotype.total_mass` |
| Tỉ lệ thân | non-uniform scale theo trục | `axial_len`, `mean_radius` |
| Màu chính xác | tint vào material | `display_color` (RGB thật, không phải tên band) |
| Tư thế/chuyển động | skeleton từ node graph + CPG | `nodes`/`edges` |
| Biến thiên nhỏ | jitter tất định theo `agent_id` | hash, không RNG toàn cục |

Vì vậy hai con cùng archetype vẫn có **số đo khác nhau và collider khác nhau**; chỉ mặt phẳng
hiển thị dùng chung. Đây chính là P06 nhìn từ phía lợi ích.

**Hệ quả chi phí.** Số lần gọi API trong một run bị chặn bởi số archetype *thực sự xuất hiện*,
không bởi số cá thể — cộng thêm trần cứng `max_archetypes_per_run` (P09). Với trần mặc định 200,
chi phí một run là **≤ 200 lần gọi**, một lần duy nhất, và các run sau dùng lại kho.

**Hệ quả độ trễ.** Sinh vật xuất hiện ngay bằng mesh procedural và **đổi sang asset khi asset
sẵn sàng**. Không có cá thể nào chờ mạng (P07).

---

## 9. Cổng nghiệm thu asset

Prompt không bảo đảm đặc điểm đúng; cổng này mới bảo đảm. Chạy trên `.glb` trả về, **trước** khi
ghi vào kho.

| Cổng | Kiểm | Hành động khi trượt |
|---|---|---|
| **A1** | Chuẩn hoá scale về chiều cao mét thật của phenotype | luôn chạy, không phải cổng |
| **A2** | Aspect ratio bounding box lệch ≤ 25% so với phenotype | retry seed khác |
| **A3** | Đối xứng: gương qua mặt phẳng dọc, Chamfer distance dưới ngưỡng | retry |
| **A4** | Số tam giác ≤ ngân sách sau decimation | decimate, rồi kiểm lại |
| **A5** | Số connected component ≤ 2 | retry |
| **A6** | Tỉ lệ mặt suy biến/không hợp lệ dưới ngưỡng | retry |
| **A7** | Auto-skin vào bone graph của phenotype thành công, ≤ 4 bone/vertex | retry |
| **A8** | Duyệt thủ công một lần cho mỗi archetype (tuỳ chọn, có thể tắt) | đánh dấu `unreviewed` |

Chính sách retry: tối đa `K` seed (mặc định 3). Hết K mà vẫn trượt ⇒ **mesh procedural**, ghi
`ArchetypeKey` kèm lý do trượt vào sổ. Không có nhánh nào dẫn tới "spawn một con vật không có
hình".

A7 đáng chú ý: skeleton đã biết trước từ node graph, nên mesh sinh ra không cần rig kèm theo —
ta bind nó vào bộ xương engine đã có và nó cử động bằng chính CPG đang chạy.

---

## 10. Gate của hợp đồng

| Gate | Bằng chứng |
|---|---|
| **PR-S01** | Cùng `MorphoSpec` ⇒ cùng prompt, byte-exact, qua nhiều lần chạy và nhiều thứ tự lặp |
| **PR-S02** | Mọi prompt sinh từ fixture quần thể đều ≤ 1000 ký tự; p95 ≤ 700 |
| **PR-S03** | Cắt theo ngân sách chỉ bỏ slot tuỳ chọn, không bao giờ tạo mệnh đề cụt |
| **PR-S04** | Mọi nhánh lexicon đều có ít nhất một fixture chạm tới (không có band chết) |
| **PR-S05** | `ArchetypeKey` đổi khi và chỉ khi prompt/negative/params/model/seed đổi |
| **PR-S06** | Đổi `Lexicon.version` làm mới toàn bộ key; asset cũ không bị coi là hợp lệ cho key mới |
| **PR-S07** | Prompt không chứa thuộc tính nào đã có tham số riêng (P03), kiểm bằng danh sách cấm |
| **PR-S08** | Xoá kho asset ⇒ checksum run không đổi (P08); app vẫn chạy đủ (P07) |
| **PR-S09** | Trần `max_archetypes_per_run` được tôn trọng dưới quần thể tổng hợp 10⁵ cá thể |
| **PR-S10** | Style lock giống hệt nhau trên toàn bộ prompt của một run |

PR-S04 là cổng dễ bị bỏ nhất và có giá trị cao nhất: một band lexicon không fixture nào chạm tới
là một nhánh chưa ai từng nhìn thấy đầu ra, và nó sẽ nằm im tới lúc một run thật rơi vào đó.

---

## 11. Ví dụ đã lắp ráp

Năm prompt dưới đây sinh từ đúng lexicon §5, không sửa tay. Cột ký tự là số đo thật.

### A · `BorealForest`, `Predator`, large, stocky quadruped, armor trait cao

```text
a large stocky four-legged creature, deep barrel torso, four short limbs ending in clawed paws, thick shaggy fur, muted dark olive, broad head with forward-facing eyes and short powerful jaws, small rounded ears and a short thick tail, bony dorsal plates, stylized realistic creature design, symmetrical anatomy, neutral A-pose, single full-body creature, plain empty background, even matte PBR texture
```

**402 ký tự** — dư 598 dưới trần.

### B · `Desert`, `Prey`, small, slender quadruped

```text
a small slender four-legged creature, long low body, four stilt-like limbs ending in blunt hooved feet, dry keratinous scales, pale ochre, sand-toned countershading, narrow head with wide-set lateral eyes and a blunt grazing muzzle, large radiating ears and a long thin tail, stylized realistic creature design, symmetrical anatomy, neutral A-pose, single full-body creature, plain empty background, even matte PBR texture
```

**422 ký tự** — dư 578 dưới trần.

### C · `Ocean`, `Predator`, medium, finned, tốc độ cao

```text
a medium-sized slender finned aquatic creature, torpedo-shaped body, four webbed paddle fins, smooth slick skin, muted slate-blue, broad head with forward-facing eyes and short powerful jaws, deep chest and long hind limbs, stylized realistic creature design, symmetrical anatomy, neutral A-pose, single full-body creature, plain empty background, even matte PBR texture
```

**370 ký tự** — dư 630 dưới trần.

### D · `Rainforest`, `Prey`, tiny, serpentine — không chi, không Allen

```text
a tiny slender serpentine creature, long tapering body, glossy moist skin, moss-green, dappled bark-toned markings, narrow head with wide-set lateral eyes and a blunt grazing muzzle, stylized realistic creature design, symmetrical anatomy, neutral A-pose, single full-body creature, plain empty background, even matte PBR texture
```

**329 ký tự** — dư 671 dưới trần.

### E · `Snow`, `Prey`, massive quadruped, thế giới có `ExoticEnergy`

```text
a massive stocky four-legged creature, deep barrel torso, four stubby limbs ending in blunt hooved feet, thick shaggy fur, bone-white, pale mottled winter coat, narrow head with wide-set lateral eyes and a blunt grazing muzzle, small rounded ears and a short thick tail, faint bioluminescent markings, stylized realistic creature design, symmetrical anatomy, neutral A-pose, single full-body creature, plain empty background, even matte PBR texture
```

**448 ký tự** — dư 552 dưới trần.

Prompt dài nhất trong bộ này là **448 ký tự**, tức khoảng **44%** trần. Khoảng dư đó là
tài sản, không phải chỗ trống cần lấp: nó cho phép lexicon mô tả cụ thể hơn ở các slot đã có khi
đo được rằng cụ thể hơn cho kết quả tốt hơn, mà không phải mở lại ngân sách.

Ba điều đọc được từ bộ ví dụ:

- **Ví dụ D không có slot chi và không có slot Allen.** Sinh vật không chân ở vùng ôn hoà thì
  hai slot đó vắng mặt, không phải điền `no limbs` — một mệnh đề phủ định trong prompt thường
  bị model dịch thành sự hiện diện của chính thứ bị phủ định. Phủ định thuộc về negative prompt.
- **Ví dụ E là con duy nhất có `faint bioluminescent markings`,** vì nó là con duy nhất biểu hiện
  `ExoticEnergy`. Một luật thế giới khác ([ADR-0002](../decisions/ADR-0002-world-laws-and-exotic-energy.md))
  đi thẳng ra ngoại hình mà không cần thêm slot nào.
- **Mọi ví dụ kết thúc bằng đúng một chuỗi style lock.** Đó là P05 nhìn từ đầu ra.
