---
title: Tạo hình Bề mặt, Hoạt hình Thủ tục và Sắc tố Sinh học cho Sinh vật Đa hình thái trên iGPU
status: proposed
owner: architecture
last_reviewed: 2026-11-22
review_cycle: quarterly
---

# Tạo hình Bề mặt, Hoạt hình Thủ tục và Sắc tố Sinh học cho Sinh vật Đa hình thái trên iGPU

> **Phạm vi & tính chất tài liệu.** Tài liệu này là **tổng hợp nghiên cứu ngoài kết hợp đối chiếu mã nguồn & hợp đồng Anima Engine** cho lộ trình nâng cấp hình thái sinh vật sau Thẻ B1 ([`2026-11-19-feature-real-world-and-bodies.md`](../ai/design/2026-11-19-feature-real-world-and-bodies.md)) sang pha renderer `wgpu` (`NR-SPIKE` / `ADR-0011` — **ADR-0011 chưa tồn tại**, mới là việc dự kiến trong [`AGENT_WORKFLOW.md`](../how-to/AGENT_WORKFLOW.md)). Đây **không** phải hợp đồng tiến hoá hay số đo hiệu năng thực tế trên phần cứng dự án; trạng thái đo được nằm duy nhất tại [`STATE_OF_THE_PROJECT.md` §1](../planning/STATE_OF_THE_PROJECT.md#1-bảng-bằng-chứng-có-thẩm-quyền).
>
> **Cảnh báo phân loại bằng chứng (`2026-11-22`).** Các ước lượng tổng hợp từ báo cáo gốc như **80,1 MFLOPs** lúc sinh, **131,2 MB VRAM** và **2,48 ms trên Intel Iris Xe G7** (hay **1,82 ms** Radeon 780M, **1,54 ms** Radeon 890M) đều là **kết quả suy dẫn từ mô hình ngân sách phần cứng ngoài** (nguồn nội bộ chưa công bố liên kết đã được loại khỏi mục tham khảo). Do chưa có benchmark công bố đo trọn gói 1.000 cá thể đột biến độc bản trên iGPU thật, mọi con số ngân sách tổng hợp mang nhãn **`Mô hình ngoài / chưa đo trên Anima`** và cần đo trực tiếp khi dựng nguyên mẫu.

---

## 0. Đối chiếu Anima Engine & bốn cảnh báo kiến trúc – pháp lý (đối chiếu mã 2026-11-22; cảnh báo pháp lý ở mục 1 **chưa xác minh**)

0. **Ràng buộc gốc từ Thẻ B1 và ADR-0001 (`Đã kiểm` với mã và tài liệu, 2026-11-22):**
   - [`2026-11-19-feature-real-world-and-bodies.md`](../ai/design/2026-11-19-feature-real-world-and-bodies.md) §3 là nguyên tắc **không thương lượng**: *"Chỉ vẽ cái mô phỏng có"*; hình dạng sinh vật đến từ dữ liệu Rust gửi; chuyển động trang trí (thở, nhún) xác định theo `agent_id` và **không thay tư thế backend**; lớp trang trí phải được đánh dấu là trang trí và không được trông giống dữ liệu. [`ADR-0001`](../decisions/ADR-0001-creature-development-lifecycle.md) (mục "Động lực quyết định") yêu cầu geometry, physics, renderer và MAP-Elites **đọc cùng phenotype**.
   - Tầng vật lý của Anima là **chuỗi đốt** (`Segment`, `Position`, `Rotation`, `ParentLink`, `JointAxis` trong [`core/components.rs`](../../src-tauri/src/core/components.rs); `resolve_joints_system` / `integrate_physics_system` trong [`physics/dynamics.rs`](../../src-tauri/src/physics/dynamics.rs)), mỗi đốt mang một `CpgOscillator` thật ([`ai/cpg.rs`](../../src-tauri/src/ai/cpg.rs)). Mỗi tick backend gửi `SegmentState` (vị trí, `yaw`/`pitch`/`roll`, `length`, `radius`, neo khớp, trục khớp — [`core/simulation_state.rs`](../../src-tauri/src/core/simulation_state.rs), kiểu TS sinh ở [`SegmentState.ts`](../../src/types/generated/SegmentState.ts)). Payload **không** có góc khớp hay pha CPG (B1 design §2), nên CPG đến renderer qua tư thế từng đốt.
   - Hệ quả cho tài liệu này: renderer **vẽ tư thế từng đốt mà backend gửi**; mọi chân, IK, đặt chân hay đuôi thủ tục ở Mục 2 hoặc là **lớp trang trí được đánh dấu rõ**, không bao giờ thay tư thế backend, hoặc là **việc tương lai cần đổi mô phỏng trước**. Renderer **không** được chọn gốc hay lọc nút khác với cơ thể mà `decode_genotype` dựng ([`evolution/genotype.rs`](../../src-tauri/src/evolution/genotype.rs), gốc = nút đầu tiên không có cạnh vào, duyệt BFS).
1. **Cảnh báo bằng sáng chế đối với `Optimized Centers of Rotation (CoR)` (`Chưa xác minh` + `Cảnh báo pháp lý`):**
   - Báo cáo kỹ thuật ngoài đề xuất dùng **Optimized CoR** (Le & Hodgins, SIGGRAPH 2016 [9]) cho `LOD0`. Nguồn duy nhất cho tuyên bố "Disney Research nắm giữ bằng sáng chế CoR" là **một bài đăng diễn đàn Unity** [10], **không nêu số hiệu bằng sáng chế**; tra cứu web ngày 2026-11-22 không tìm thấy số hiệu nào. Đây là tuyên bố chưa xác minh, không phải kết luận pháp lý.
   - **Khuyến nghị cho Anima:** chưa đặt CoR làm mặc định; mặc định kỹ thuật cho `LOD0`/`LOD1` là **Dual-Quaternion Skinning (DQS — 32 B/xương)**. Tình trạng bằng sáng chế của **cả CoR lẫn DQS** đều chưa được rà soát; cần kiểm tra pháp lý trước khi dựa vào bất kỳ phương án nào cho bản phát hành. Việc bỏ trường vector lệch tâm quay `cor_and_flags` (`8 B`) thu gọn đỉnh từ `32 byte` xuống **`24 byte/đỉnh`**.
2. **Hợp nhất VRAM & bộ nhớ đệm `morphologyKey` (`Đã kiểm` + `Suy luận`):**
   - Trong [`creatureSkinMesh.ts`](../../src/components/Landscape/utils/creatureSkinMesh.ts) (dòng 15 và 18), nguyên mẫu B1 hiện dựng lưới da liên tục bằng `SKIN_RADIAL_SIDES = 8` và đã có sẵn **`MAX_ARCHETYPE_CACHE = 128`** bộ đệm theo `morphologyKey`. Trần tam giác sinh vật theo tier nằm ở [`qualityTiers.ts`](../../src/components/Landscape/utils/qualityTiers.ts): Low `300_000` (dòng 64), Medium `400_000`, High `600_000` (dòng 110).
   - Khác biệt ngân sách so với báo cáo ngoài: báo cáo ngoài thiết kế cho trần **60 FPS (`16,67 ms`)** với **1.000 cá thể độc bản 100%** tốn **`131,2 MB` VRAM** (`72,63 MB` VBO + `25,29 MB` IBO + `31,25 MB` Atlas `2048²`). Trong khi đó, hồ sơ Low Intel Iris Xe của Anima ([`2026-11-19-feature-real-world-and-bodies.md`](../ai/design/2026-11-19-feature-real-world-and-bodies.md) & [`2026-11-20-world-v2-design.md`](../ai/design/2026-11-20-world-v2-design.md)) nhắm **30 FPS (`33,3 ms`)** và phải chừa băng thông + VRAM cho địa hình/thảm thực vật (khoảng `~35–55 MiB` VRAM cho bản đồ v2 Low).
   - **Hành động hợp nhất:** Chia sẻ lưới theo `morphologyKey` khi nhiều cá thể cùng nguyên mẫu, hoặc thu nhỏ **Dynamic Billboard Atlas xuống `1024²`** trên cấu hình Low thay vì cố giữ `2048²`. *(Suy luận số học: `1024² / 64² = 256` ô, khoảng `8 MiB` với hai lớp RGBA8 — **ít hơn** ~300 cá thể mà bảng LOD ở Mục 4.2 xếp vào `LOD3`; vì vậy hoặc giảm ô xuống `32²`–`48²`, hoặc giới hạn số cá thể `LOD3` có ô riêng, phần còn lại dùng marker xa như renderer hiện tại.)*
3. **Tách rạch ròi nâng cấp hiển thị thuần tuý (*cosmetic*) vs nâng cấp chạm hợp đồng tiến hoá (`Đã kiểm` với nguyên tắc B1):**
   - **Thuần hiển thị, vẫn vẽ đúng tư thế backend:** chuyển chuỗi capsule sang **Naive Surface Nets** + `sdRoundCone` trên **đúng cây đốt mà `decode_genotype` dựng**, tự sinh trọng số cách ly theo đồ thị đốt, biến dạng DQS điều khiển bởi `SegmentState` từng tick, và Mega-Buffer nhiều tầng LOD. Các bước này không đổi quỹ đạo vật lý hay genome.
   - **Lớp trang trí được đánh dấu (không thay tư thế backend):** chuyển động phụ xác định theo `agent_id` (thở, nhún, lắc nhẹ phần chóp) cộng thêm lên tư thế backend với biên độ nhỏ, có công tắc tắt và không trông giống dữ liệu — đúng điều B1 §3 cho phép.
   - **Việc tương lai cần đổi mô phỏng trước:** chân có bàn chân, đặt chân bám dốc, IK, đuôi Verlet có động lực riêng, dáng đi CPG nhiều chân hay sóng serpenoid khác với chuyển động thật của chuỗi đốt. Anima hiện **không có** chân hay bàn chân trong mô phỏng; vẽ chúng là vẽ thứ mô phỏng không có. Chúng chỉ được vẽ sau khi mô phỏng sinh ra dữ liệu tương ứng (xem [`ADR-0006`](../decisions/ADR-0006-locomotion-actuation.md) về vận động).
   - **Chạm hợp đồng / schema bộ gen:** bổ sung gen sắc tố mới (`g_F, g_Delta_k`, melanin/carotenoid/iridophore có tác dụng sinh thái) hoặc thẻ năng lực cấu trúc (*capabilities* ảnh hưởng tốc độ/chiến đấu) bắt buộc đi qua cập nhật [`CREATURE_DEVELOPMENT_CONTRACT.md`](../reference/CREATURE_DEVELOPMENT_CONTRACT.md) và [`ADR-0001-creature-development-lifecycle.md`](../decisions/ADR-0001-creature-development-lifecycle.md) sau khi đóng Chặng 4.

---

## 1. Bọc da liền mạch và Tự sinh Bộ xương – Trọng số từ Đồ thị Đốt–Khớp

### 1.1. Trải phẳng Đồ thị Gen thành Cây Xương Động học và Tham số hoá Sinh học

**Genotype thật của Anima (`Đã kiểm`, [`evolution/genotype.rs`](../../src-tauri/src/evolution/genotype.rs) dòng 5–24):** `MorphologyGenotype` gồm `nodes: Vec<MorphologyNode>` (`id`, `length`, `radius`, `mass`) và `edges: Vec<MorphologyEdge>` (`source_node`, `target_node`, `joint_anchor`, `joint_axis`). Nó **không có** cờ đối xứng phản chiếu, vòng lặp đệ quy hay giới hạn khớp như genotype đồ thị có hướng của Karl Sims (1994) [30, 31]. `decode_genotype` chọn gốc là nút đầu tiên không có cạnh vào và dựng thân bằng BFS. Mục dưới đây vì vậy chia hai phần: phần áp dụng cho genotype hiện có, và phần **mở rộng giả định** (chỉ là ý tưởng, cần đổi genotype và hợp đồng trước).

Áp dụng cho genotype hiện có, ở **pha dựng lưới lúc sinh (`Birth Compute Pass`, renderer)**:

1. **Dùng nguyên cây đốt của `decode_genotype`:** cùng gốc, cùng tập nút đạt được, cùng thứ tự BFS. Renderer **không** chọn gốc theo heuristic riêng và **không** lọc bỏ nút, để hình vẽ và cơ thể vật lý là một (ADR-0001: renderer đọc cùng phenotype). Nếu cần ít xương hơn ở LOD xa, gộp xương chỉ ở **mức lưới hiển thị của tầng LOD đó**, giữ ánh xạ ngược về `SegmentState` từng đốt.

Mở rộng giả định (**không** có trong code; cần thay `MorphologyGenotype`, [`CREATURE_DEVELOPMENT_CONTRACT.md`](../reference/CREATURE_DEVELOPMENT_CONTRACT.md) và ADR-0001 trước khi dùng):

2. **Trải phẳng chu trình thành Tree/DAG** với độ sâu đệ quy bị chặn bởi gen, theo kiểu Sims [30] và Spore [17]. Khi nhân bản cạnh mang cờ đối xứng phản chiếu, có thể cấp phát kênh điều khiển độc lập cho nhánh đối diện để hai chi đối xứng bước lệch pha được (`Suy luận`; nguồn [32] là bài của tác giả khác về mô hình sinh vật ảo, không phải bài tiếp nối của Sims).
3. **Tham số hoá tỷ lệ sinh học cấp cao (Rune Skovbo Johansen 2021–2024):** tầng giải mã gen biểu diễn tỷ lệ có nghĩa (độ thon thân, tỷ lệ chiều dài chi trên khoảng sáng gầm, góc gập gối ưu tiên) rồi hạ xuống nút–cạnh [13].
4. **Lọc bớt nút trung gian thẳng hàng theo Zhu et al. [33]** — chỉ hợp lệ nếu phép lọc nằm **trong** phát triển lúc sinh, tức vật lý cũng dùng cơ thể đã lọc; renderer không được lọc riêng. Tiêu chí: loại nút bậc 2 không phân nhánh thoả đồng thời góc lệch gần phẳng và đoạn ngắn:
   $$
   \angle N_{k-1} N_k N_{k+1} < \varepsilon \qquad \text{và} \qquad \min(\|N_{k-1}N_k\|,\, \|N_k N_{k+1}\|) < c\,r_k
   $$
   (với $r_k$ là bán kính nút), giữ số xương hoạt động gọn trong ngưỡng **16–48 xương** (dưới giới hạn mảng hằng 64 xương truyền thống [34]).

### 1.2. Thiết kế Bề mặt Tiềm ẩn: Convolution Surfaces, Tapered SDF và `B-Mesh`

Metaball điểm dọc thân gây gợn sóng và **phình cục (*bulging*)** khi nhiều nhánh hội tụ ở vai/hông [35, 37]. Ba hướng thay thế chính gồm:

1. **Line-Segment Convolution Surfaces (Bloomenthal & Shoemake / McCormack & Sherstyuk [36, 37, 41]):**
   Tích phân trường $F(\mathbf{p}) = (f \otimes g)(\mathbf{p})$ dọc đoạn thẳng xương mang lại **tính bất biến tuyệt đối với phép chia nhỏ xương** và hoà trộn trơn $C^\infty$ ở chạc phân nhánh.
   - Với **nhân Cauchy hỗ trợ vô hạn** $\kappa(r) = (1 + \alpha^2 r^2)^{-2}$ dọc đoạn $[\mathbf{p}_0, \mathbf{p}_1]$ dài $L = \|\mathbf{p}_1 - \mathbf{p}_0\|$ ($d = \|\mathbf{p}-\mathbf{p}_0\|$, $h = (\mathbf{p}-\mathbf{p}_0)\cdot(\mathbf{p}_1-\mathbf{p}_0)/L$, $a = \sqrt{1+\alpha^2(d^2-h^2)}$), tồn tại nghiệm giải tích đóng dùng hàm $\arctan$, nhưng đuôi dương vô hạn khiến các chi đứng gần (ngón chân/hai đùi) dễ **dính màng** ngoài ý muốn [39, 41].
   - Để cô lập cục bộ, **nhân đa thức bậc 4 hỗ trợ hữu hạn** của Jin et al. (2009) $f_{\text{Quartic}}(r) = (1 - r^2/R_i^2)^2$ ($r \le R_i$, bằng $0$ khi $r > R_i$, chọn $R_i = 2d_i$) triệt tiêu ảnh hưởng ngoài bán kính $R_i$ [38], kết hợp nghiệm đóng trọng số $\lambda_i$ của Zhu et al. không cần giải NNLS toàn cục và phép co giãn cục bộ $(\alpha, \beta_x, \beta_y)$ vuông góc trục xương để tạo ngực dẹp hay vây cá [38].
2. **Tapered Capsule SDF (`sdRoundCone`) + hàm `smin` có điều kiện theo DAG (Khuyến nghị tốc độ cao GPU):**
   Theo mô hình *Sphere-Meshes* (Thiery, Guy & Boubekeur 2013 [42, 43]), đoạn nối hai nút cầu là bao lồi $\bigcup_{u\in[0,1]} S(u\mathbf{q}_i+(1-u)\mathbf{q}_j,\, ur_i+(1-u)r_j)$, tính chính xác bằng `sdRoundCone` của Inigo Quilez (~28 FLOPs, **không dùng `atan`** [44]). Khi trộn theo toán tử cực tiểu trơn đa thức bậc hai [3]:
   $$
   h = \text{clamp}\!\left(\frac{1}{2} + \frac{d_B - d_A}{2k},\, 0,\, 1\right), \qquad \text{smin}(d_A, d_B, k) = (1 - h)d_B + h d_A - k h(1 - h)
   $$
   bằng cách **chỉ áp dụng `smin` giữa cặp xương cha–con hoặc anh em chung khớp trên DAG** và dùng `min` cứng giữa các nhánh xa nhau topo, hệ thống giảm mạnh lỗi hai chi đứng gần chảy dính vào nhau (`Suy luận`: `smin` không kết hợp, nên kết quả phụ thuộc thứ tự gộp theo cây, và `min` cứng để lại nếp gấp nơi hai nhánh xa thật sự giao nhau).
3. **`B-Mesh` (Ji, Liu & Wang 2010 — nền tảng *Blender Skin Modifier* [45, 46, 48]):**
   Quét mặt cắt vuông + dựng bao lồi quanh nút nhánh + khâu ống + làm mịn Catmull–Clark (bù co rút thể tích $\alpha_1 = \frac{3}{2}\gamma$ [48]). Dù tạo *edge-flow* đẹp, thực tế phát triển *Skin Modifier* ghi nhận góc nhánh chụm hẹp dễ tạo mặt tự cắt (*folding*) hoặc lỗi không đa tạp [46].

**Đối chiếu ba thuật toán trích xuất mặt đẳng trị (`Nguồn ngoài`):**

| **Tiêu chí kỹ thuật** | **Marching Cubes** (1987) [51] | **Naive Surface Nets** *(Khuyến nghị)* [4, 50, 53] | **Dual Contouring / ODC** (2002 / 2024) [55, 56] |
|---|---|---|---|
| **Vị trí đỉnh & topo** | Trên cạnh ô lưới; bảng 256 cấu hình; sinh tam giác [51] | Trong ô lưới (trung bình giao điểm 12 cạnh); sinh **tứ giác** [50] | Trong ô lưới; giải QEF từ giao điểm & pháp tuyến Hermite [51] |
| **Số đa giác (benchmark `0fps.net`, tần số nhiễu 1 → 10)** | `15.520 → 154.168` tam giác; hay tạo tam giác kim mảnh | **`7.569 → 73.133` tứ giác** — mỗi tứ giác là 2 tam giác, nên số **tam giác** tương đương ~`15,1 nghìn → 146,3 nghìn`, xấp xỉ Marching Cubes chứ không ít hơn một nửa; lợi thế thật là ít tam giác kim mảnh | Tương đương Surface Nets; hỗ trợ octree thích nghi |
| **Tốc độ Compute GPU** | Rất nhanh, độc lập từng ô [52] | Nhanh hơn rõ rệt ở hình học dày nhờ phép trung bình đơn giản | Đòi hỏi đạo hàm Hermite + QEF; biến thể ODC 2024 chậm hơn MC `1,8–4,0×` |
| **Chất lượng hình học** | Vát tròn góc nhọn; có ca nhập nhằng topo | Bề mặt hữu cơ mượt, không bảng tra; vẫn cần lưu ý ca suy biến hiếm | Giữ cạnh giáp cứng tốt; QEF dễ bất ổn khi pháp tuyến song song |

*Phương án tối ưu:* đánh giá `sdRoundCone + smin` (hoặc Quartic Convolution) trên lưới cục bộ và trích xuất bằng **Naive Surface Nets**; các chi tiết sừng hoá sắc cạnh (mắt, răng, móng, gai) gắn dưới dạng lưới con tham số hoá dọc pháp tuyến bề mặt.

### 1.3. Tự sinh Trọng số Đỉnh Không Giải Hệ Toàn cục

Các phương pháp DCC (*Bounded Biharmonic Weights* quá chậm cho runtime, *Pinocchio Heat Diffusion* sập ở lưới không đa tạp, *Geodesic Voxel Binding* phải voxel hoá ngược lưới [6]). Khi da sinh trực tiếp từ trường xương, rút trích trọng số trong 3 bước ngay tại Compute Shader lúc sinh [2, 5, 43]:

1. **Đọc đóng góp trường tại đỉnh $\mathbf{v}_i$:** Dùng trực tiếp thế năng riêng phần $\tilde{w}_{ij} = \lambda_j F_j(\mathbf{v}_i)$ (nếu dùng Convolution Surface) hoặc hàm suy giảm khoảng cách chuẩn hoá từ `sdRoundCone`.
2. **Cách ly rò rỉ xuyên không gian bằng khoảng cách bước nhảy DAG:** Gán xương chủ đạo $j^* = \arg\min_j d_j(\mathbf{v}_i)$ và chỉ chấp nhận $\tilde{w}_{ij} > 0$ từ các xương $j$ thoả:
   $$
   \text{dist}_{\text{DAG}}(j^*, j) \le 2
   $$
   (xương chủ đạo, cha/ông, con và anh em chung khớp), ngăn lỗi một chi hút nhầm xương của chi bên cạnh mà không cần Dijkstra 3D. Đồ thị ở đây là cây đốt do `decode_genotype` dựng.
3. **Chọn Top-4 & khuếch tán Jacobi cục bộ:** Giữ 4 trọng số lớn nhất, chuẩn hoá tổng bằng `1`; với khớp nhiều nhánh, chạy 2–3 lượt lặp Jacobi song song của hệ khuếch tán nhiệt $(-\Delta + H)\mathbf{w}_i = H\mathbf{p}_i$. Phương trình này là của **Pinocchio (Baran & Popović, SIGGRAPH 2007)**, không phải của [43]; ở đây chỉ dùng cục bộ, vài lượt lặp, nên tránh được nhược điểm lưới không đa tạp nêu ở trên (`Suy luận`, chưa đo). Dấu của $\Delta$ tuỳ quy ước Laplacian.

### 1.4. Biến dạng Khung xương Thời gian thực: LBS, SBS, DQS và CoR

Trong biến dạng **Dual-Quaternion Skinning (DQS — Kavan et al. 2007/2008 [7, 65, 71])**, mỗi phép biến đổi cứng được biểu diễn bởi quaternion kép $\hat{q} = q_0 + \varepsilon q_\varepsilon$ ($\varepsilon^2 = 0$, $q_\varepsilon = \frac{1}{2}(0,\mathbf{t})q_0$) chỉ tốn **8 float (`32 B/xương`)** — giảm **33,3%** băng thông xương so với ma trận 3×4 của LBS (`48 B/xương`). Sau khi đồng bộ dấu bán cầu $s_k = \text{sign}(\langle q_{0,j_1}, q_{0,j_k}\rangle)$ để tránh xoay ngược vòng cung và tổ hợp $\hat{b} = b_0 + \varepsilon b_\varepsilon = \sum_{k=1}^4 w_{ik} s_k \hat{q}_{j_k}$, phép biến đổi đỉnh tối ưu trong Vertex Shader là:

$$
\mathbf{q}_r = \frac{b_0}{\|b_0\|}, \qquad \mathbf{t}_r = \frac{2\,\text{vec}(b_\varepsilon b_0^*)}{\|b_0\|^2}, \qquad \mathbf{v}'_i = \mathbf{v}_i + 2\,\mathbf{q}_{r,\text{vec}} \times \big(\mathbf{q}_{r,\text{vec}} \times \mathbf{v}_i + q_{r,w}\mathbf{v}_i\big) + \mathbf{t}_r
$$

| Toán tử Skinning | Hằng số xương / frame | Thuộc tính đỉnh phụ | Hành vi tại khớp (gập > 90° & xoắn 180°) | Đánh giá hiệu năng & vai trò cho Anima |
|---|---|---|---|---|
| **LBS** [7] | `12 float (48 B)` | `0 B` | Sụp thể tích khuỷu/gối; xoắn kẹo 180° sụp về 1 điểm | Số đo trong Kavan [7] (`0,32 ms` GPU, `3,67 ms` CPU, chưa xác minh lại) là trên phần cứng năm 2007, **không áp dụng** cho iGPU hôm nay; dùng cho **`LOD2`** (`1–2` trọng số) |
| **Spherical Blend (SBS)** [7] | Ma trận + Tâm quay cụm SVD | Chỉ số cụm đỉnh | Chống xoắn nhưng lộ vết rách tại ranh giới cụm | Kavan [7] báo chậm hơn DQS khoảng 2× (phần cứng 2007, chưa xác minh lại) → **Loại bỏ** |
| **Dual-Quaternion (DQS)** [7, 65] | **`8 float (32 B)`** | `0 B` | Triệt tiêu xoắn kẹo; phồng nhẹ mặt ngoài khi gập gắt | Giảm 33% băng thông xương so với 3×4; thời gian trên Anima **chưa đo** → **Khuyến nghị kỹ thuật mặc định cho `LOD0` & `LOD1`** (có thể pha trộn với LBS để giảm phồng — thực hành phổ biến, [67] **không** phải nguồn cho cách này; nguồn cụ thể chưa xác minh) |
| **Optimized CoR** [9, 64] | `8–12 float` | $\mathbf{p}^*_i - \mathbf{v}_i$ (`6 B`) | Bảo toàn tiết diện ngang tốt hơn LBS/DQS theo [9] | **Tuyên bố bằng sáng chế chưa xác minh [10]** (xem Mục 0.1) → chưa dùng mặc định cho tới khi rà soát pháp lý |

*(Ghi chú: Phương pháp hậu xử lý Implicit Skinning [68, 70] đạt 35–95 FPS cho 1 nhân vật đơn trên CUDA — quá đắt cho 1.000 cá thể trên iGPU).*

---

## 2. Chuyển động: vẽ tư thế backend trước, kỹ thuật thủ tục là trang trí hoặc việc tương lai

> **Ràng buộc của mục này (xem Mục 0.0).** Renderer vẽ tư thế từng đốt mà backend gửi trong `SegmentState` mỗi tick; chuyển động đó đã là kết quả của `CpgOscillator` thật ([`ai/cpg.rs`](../../src-tauri/src/ai/cpg.rs)) qua `resolve_joints_system` / `integrate_physics_system` ([`physics/dynamics.rs`](../../src-tauri/src/physics/dynamics.rs)). Việc của renderer là nội suy giữa hai tick và dựng da theo tư thế đó. Các kỹ thuật dưới đây (serpenoid, CPG nhiều chân, đặt chân, IK, đuôi Verlet) **không được thay tư thế backend**. Mỗi kỹ thuật được gắn một trong hai nhãn: **[Trang trí]** — cộng thêm lên tư thế backend, xác định theo `agent_id`, biên độ nhỏ, có công tắc tắt, không trông giống dữ liệu; hoặc **[Tương lai — cần đổi mô phỏng]** — chỉ vẽ được khi mô phỏng sinh ra dữ liệu tương ứng (ví dụ chân thật, góc khớp, pha CPG trong payload). Anima hiện không có chân hay bàn chân trong mô phỏng.
>
> Phía mô phỏng của việc "sinh vật tự đi bằng cơ thể" (CPG sóng lan truyền, ma sát dị hướng, Phương án B của ADR-0006) được đánh giá riêng ở [`BRAIN_BODY_COEVOLUTION_EVODEVO_LOCOMOTION_RESEARCH_2026-11-22.md`](BRAIN_BODY_COEVOLUTION_EVODEVO_LOCOMOTION_RESEARCH_2026-11-22.md) §3.

### 2.1. Tách lớp Vật lý – Hình ảnh: kỹ thuật tham khảo (*Rain World* & *Spore*) [11, 17, 72, 74]

Trong *Rain World*, mô phỏng chỉ cập nhật vài khối cầu va chạm (*body chunks* — thuật ngữ của Rain World; Slugcat có 2 khối [11]) và hoạt hình thủ tục dựng phía trên. **Anima không có tầng "Body Chunks".** Tầng vật lý của Anima là **chuỗi đốt** (`Segment`, `Position`, `Rotation`, `ParentLink`, `JointAxis` trong [`core/components.rs`](../../src-tauri/src/core/components.rs)), và renderer đã nhận tư thế từng đốt chứ không chỉ một gốc.

- **Áp dụng được ngay:** đặt độ cao gốc theo mặt đất của frontend (B1 §2: chỉ x/z đến từ mô phỏng, y lấy từ mặt đất frontend) và có thể nghiêng nhẹ cả thân theo pháp tuyến địa hình bằng raycast chỉ-đọc:
  $$
  T_{\text{vis\_root}} = T_{\text{phys\_root}} \cdot \Delta T_{\text{terrain\_offset}}
  $$
  Độ lệch này chỉ cộng vào gốc hiển thị, giữ nguyên tư thế tương đối giữa các đốt mà backend gửi và không ghi gì về Rust sim.
- **Không áp dụng:** một "rig thủ tục" tự dựng chân bước và đuôi thay cho tư thế đốt của backend — trái B1 §3.

### 2.2. Nhịp bước (CPG, đường cong Hirose) — **[Tương lai — cần đổi mô phỏng]**, phần nhỏ **[Trang trí]**

Chuyển động thật của thân đã đến từ CPG backend qua tư thế đốt. Tạo một nhịp bước khác ở renderer sẽ vẽ chuyển động mô phỏng không có. Hai kỹ thuật sau chỉ là tư liệu cho khi mô phỏng đổi (ví dụ sóng lan truyền lệch pha mà [`ADR-0006`](../decisions/ADR-0006-locomotion-actuation.md) nhắc tới), hoặc cho một lớp trang trí biên độ nhỏ ở phần chóp:

1. **Sinh vật không chân thân dài (rắn / lươn):** **đường cong Serpenoid của Hirose** [75, 76]; dạng rời rạc dưới đây trên $n$ đốt chiều dài $L$ khớp theo vận tốc tịnh tiến $v$:
   $$
   \theta_i(t) = \alpha_w \sin(\omega t + \beta i) + \gamma, \quad i = 1,\dots,n-1, \qquad
   \alpha_w = -2\alpha_0\sin\!\left(\frac{K\pi}{n}\right), \quad \omega = \frac{2K\pi}{L}v, \quad \beta = \frac{2K\pi}{n}
   $$
   Hoặc dùng heuristic *pseudo-feet* của *Spore* [17] cho dáng bò sâu đo (*inch-worm*).
2. **Sinh vật nhiều chân bằng mạng dao động pha (kiểu Kuramoto) — tham khảo Namura & Nakao 2024 [14, 15]:**
   Bài [14, 15] dùng **sáu** bộ dao động chu kỳ giới hạn (FitzHugh–Nagumo) ghép yếu cho robot **6 chân**, rút gọn bằng *phase reduction* về sáu phương trình pha rồi về hai phương trình hiệu pha nhờ đối xứng; chuyển giữa *wave*, *tetrapod* và *tripod* đạt được bằng **cách chọn hàm ghép**. Bài không xét 4 chân và không khẳng định chỉ cần đổi tần số hay *duty cycle*. Dạng phương trình pha tổng quát (`Suy luận`, không phải công thức nguyên văn của bài):
   $$
   \dot{\theta}_k = \omega_k + \sum_{j \in \mathcal{N}_{\text{CPG}}(k)} c_{kj} \sin(\theta_j - \theta_k - \phi_{kj})
   $$
   Với độ lệch pha $\phi_{kj}$ cố định, mạng khoá vào **một** mẫu pha do $\phi_{kj}$ quyết định; đổi $\omega_k$ (khi các $\omega_k$ bằng nhau) không đổi mẫu đó. Vì vậy cấu hình "chân đối bên $\phi = \pi$, chân cùng mạn $\phi = 2\pi / N_{\text{side}}$" **không** tạo được *tripod* (ở tripod, hai chân kề nhau cùng mạn lệch $\pi$); muốn chuyển *wave* ($T/6$) → *tetrapod* ($T/3$) → *tripod* ($T/2$) phải đổi $\phi_{kj}$ hoặc hàm ghép, đúng như [14, 15]. Mọi mạng như vậy chỉ được nối vào Anima khi mô phỏng có chân thật (**[Tương lai — cần đổi mô phỏng]**).

### 2.3. Đặt chân trên địa hình dốc — kỹ thuật tham khảo từ Rune Skovbo Johansen [12, 13, 16, 80] — **[Tương lai — cần đổi mô phỏng]**

Toàn bộ mục này giả định sinh vật có chân và bàn chân, điều mô phỏng Anima chưa có; nó là tư liệu cho khi đó. Các bước 1–4 tóm tắt ý trong luận văn và bài GDC của Johansen (thứ tự và cách chia bước là của tài liệu này, chưa đối chiếu lại nguyên văn); bước 5 (Verlet) **không thuộc** phương pháp của Johansen.

1. **Dự đoán vết chân & đóng/mở khoá chu kỳ độc lập (*Park / Unpark*):** Ngoại suy vị trí thân tại thời điểm chạm đất kế theo vận tốc thẳng + góc và bắn tia raycast tìm $(\mathbf{p}_{\text{next}}, \mathbf{n}_{\text{next}})$. Khi dừng lại, từng chân tự *park* khi chạm mốc *stance*; khi tăng tốc, chân *unpark* đồng bộ theo pha ảo mà không cần clip chuyển trạng thái [12].
2. **Quỹ đạo cung tiếp tuyến vượt dốc:** Dựng hai tiếp tuyến mặt đất từ $(\mathbf{p}_{\text{prev}}, \mathbf{n}_{\text{prev}})$ và $(\mathbf{p}_{\text{next}}, \mathbf{n}_{\text{next}})$ cắt trục đứng tại trung điểm bước; lấy giao điểm cao hơn $\max(h_{\text{prev}}, h_{\text{next}})$ làm vòm nền chống xuyên gờ đá, cộng chồng độ nâng pha bay [12].
3. **Bù độ cao hông thị giác không duỗi cứng khớp ($h_{\text{desiredmin}}$):**
   Với từng chân $l$, tính cao độ mong muốn tương đối $h_{\text{desired},l}$ giữ háng cách mặt cầu cổ chân đúng cự ly tư thế nghỉ. Chọn **cực tiểu** $h_{\text{desiredmin}} = \min_l h_{\text{desired},l}$ (vì gập gối sâu ở chân trên dốc tự nhiên hơn duỗi căng chân dưới dốc, trong khi hàm trung bình làm duỗi quá tầm còn hàm kẹp gây gãy đạo hàm bậc hai), kết hợp nền đỡ trọng số cosin theo pha $\tau_l \in [0, 1]$ [16]:
   $$
   w_l^{\text{sup}} = \frac{\cos(2\pi \tau_l) + 1}{2} + \epsilon, \qquad y_{\text{support}} = \frac{\sum_{l=1}^{N_{\text{legs}}} w_l^{\text{sup}} y_{\text{ground},l}}{\sum_{l=1}^{N_{\text{legs}}} w_l^{\text{sup}}}
   $$
   Nhờ đó, chân trụ ($\tau_l \approx 0$) nâng đỡ thân còn chân vung ($\tau_l = 0{,}5 \Rightarrow w_l^{\text{sup}} = \epsilon$) không kéo thân sập xuống hố. *(Johansen không cố định công thức nghiêng thân trên dốc; có thể nội suy cầu nhẹ trục đứng hiển thị về pháp tuyến trung bình có trọng số của các chân trụ).*
4. **Giải IK 2 lượt & FABRIK cho đa nhánh:** Chi 2 đốt chạy IK kép của Johansen (giải háng–cổ chân → pha trộn góc cổ chân áp mặt dốc 0% ảnh hưởng gốc khi trụ / 100% khi bay → khử lệch tâm đế → giải lại IK lần 2) [12]. Chi $\ge 3$ đốt hoặc nhánh phân chạc chuyển sang **FABRIK** (không ma trận Jacobian/SVD, trung bình trọng tâm nút nhánh $\mathbf{p}_{\text{sub}}^{\text{new}} = \frac{1}{m}\sum_{k=1}^m \mathbf{p}_{\text{sub},k}$ [81] kết hợp tiền điều kiện tư thế nghỉ & uỷ quyền đốt cha từ *Spore* [17]).
5. **Nhánh thứ cấp bằng *Verlet* (kỹ thuật riêng, không phải của Johansen):** chỉ hợp lệ như **[Trang trí]** biên độ nhỏ cộng lên tư thế backend, hoặc **[Tương lai]** khi mô phỏng có các nhánh đó. Mọi nhánh không mang mục tiêu IK (đuôi, râu, xúc tu) tự động chạy **Position Verlet** (có nhân hiệu chỉnh $\Delta t_n / \Delta t_{n-1}$ chống đổi bước FPS [18]) với ràng buộc độ dài + giới hạn góc $(\mathbf{p}_{i-1}, \mathbf{p}_i, \mathbf{p}_{i+1})$ siết chặt ở gốc và nới lỏng về chóp [83].

---

## 3. Hoạ tiết Da Sinh học Sinh từ Gen (Reaction–Diffusion & Quang phổ Sắc tố)

> **Ràng buộc.** Hiện chưa có gen sắc tố. Theo B1 §3 và D08/D09 của [`CREATURE_DEVELOPMENT_CONTRACT.md`](../reference/CREATURE_DEVELOPMENT_CONTRACT.md), màu đi theo lớp cộng băm `agent_id` cho tới khi có pigment, được phát triển và lưu tại lúc sinh, không đổi theo quần xã. Hoa văn RD gieo từ `agent_id` mà không có gen chỉ được dùng như **[Trang trí]** đánh dấu rõ, vì nó dễ bị đọc nhầm thành một tính trạng di truyền. Mục này là tư liệu cho khi gen sắc tố được thêm qua hợp đồng (Mốc 3).

### 3.1. Giải Gray-Scott RD trên Đồ thị Đỉnh Lưới & Tham số hoá Bờ Gấp [22, 23, 24, 87]

Theo phương pháp của Greg Turk (SIGGRAPH 1991) [22, 86], lưới Surface Nets `LOD0` (~1.600 đỉnh phân bố khá đều) cho phép giải hệ phương trình **Reaction–Diffusion Gray-Scott** ($U + 2V \to 3V$, với $D_v < D_u$, chuẩn $D_u \approx 0{,}16$, $D_v \approx 0{,}08$) trực tiếp trên đồ thị đỉnh bằng **Cotangent Laplace–Beltrami chuẩn hoá diện tích Voronoi** (kẹp trọng số dương để chống tam giác tù mất ổn định [23]) và lưu nồng độ $(u_i, v_i)$ thẳng vào thuộc tính đỉnh (`0 byte` texture, `0` lỗi đường may UV):

$$
\frac{\partial u}{\partial t} = D_u \Delta_S u - uv^2 + F(1 - u), \qquad
\frac{\partial v}{\partial t} = D_v \Delta_S v + uv^2 - (F + k)v
$$

$$
(\Delta_S u)_i = \frac{1}{2 A_i}\sum_{j \in \mathcal{N}(i)} \max(0,\, \cot\alpha_{ij} + \cot\beta_{ij})(u_j - u_i)
$$

Do vùng sinh hoa văn Pearson tập trung trong hành lang rất hẹp phía trên đường biên phân nhánh yên ngựa–nút $F - 4(F+k)^2 = 0 \iff k_{\text{fold}}(F) = \frac{1}{2}\sqrt{F} - F$ (lệch quá xa đường này về phía $k$ lớn thì hệ rơi về trạng thái đồng nhất $u=1, v=0$; ngưỡng cụ thể phụ thuộc $F$ — ví dụ đốm phân bào `F=0.028, k=0.062` nằm lệch khoảng `0,006` trên đường bờ gấp, nên một ngưỡng chung `0,002–0,005` là không đúng; cần thử nghiệm để chốt biên trên của dải lệch), thay vì để gen lấy mẫu $(F, k)$ tự do, ánh xạ gen chuẩn hoá $(g_F, g_{\Delta k}) \in [0, 1]^2$ dọc theo hành lang bờ gấp (`Suy luận thiết kế`):

$$
F = 0{,}014 + 0{,}048\,g_F, \qquad k = \left(\frac{1}{2}\sqrt{F} - F\right) + (0{,}0008 + 0{,}0075\,g_{\Delta k})
$$

Phép tham số hoá này quét trúng toàn bộ phổ hoa văn: sọc mê cung (`F=0.029, k=0.057`), vân giun (`0.058, 0.065`), đốm phân bào (`0.028, 0.062`), đốm báo (`0.035, 0.065`) và sóng xoắn ốc (`0.018, 0.051`). Kết hợp **tensor khuếch tán dị hướng** $\mathbf{A} = Q^\top\text{diag}(a_1^2, a_2^2)Q$ (Witkin & Kass 1991 [24]) hướng theo tiếp tuyến xương để sinh vằn sọc dọc/ngang, hoặc **RD phân tầng** (*Cascaded RD* — Turk 1991 [22]) cho hoa văn đốm báo (*rosettes*) / mạng hươu cao cổ. Vì tích phân Euler tường minh tốn $\mathcal{O}(r^4)$ theo mật độ lưới $r=1/h$ [24], chỉ giải trên lưới thô ~1.600 đỉnh lúc sinh (ước tính ngoài ~28 MFLOPs cho 500 bước).

### 3.2. Sắc tố Melanin – Carotenoid & Ánh kim Màng mỏng Airy trong Fragment Shader [25, 26, 27]

Nồng độ morphogen $(u, v)$ nội suy trên fragment điều khiển mô hình truyền qua Beer–Lambert hai chiều xuống nền tán xạ trắng hạ bì $\mathbf{R}_{\text{dermis}}$ [25, 26]:

- Hệ số hấp thụ biểu bì của **Donner & Jensen (SIGGRAPH 2006)** [26] cho eumelanin ($\sigma_a^{\text{em}}(\lambda) = 6{,}6\times 10^{10}\lambda^{-3{,}33}$), pheomelanin ($\sigma_a^{\text{pm}}(\lambda) = 2{,}9\times 10^{14}\lambda^{-4{,}75}$) và mô nền $\sigma_a^{\text{base}}(\lambda)$ theo phân suất melanin $C_m$ và tỷ lệ pha trộn $\beta_m \in [0, 1]$ được lượng giá tại 3 bước sóng đại diện `[615, 535, 455]` nm:
  $$
  \boldsymbol{\sigma}_{\text{em}} = [34{,}09;\, 54{,}22;\, 92{,}98]\ \text{mm}^{-1}, \quad
  \boldsymbol{\sigma}_{\text{pm}} = [16{,}42;\, 31{,}82;\, 68{,}68]\ \text{mm}^{-1}, \quad
  \boldsymbol{\sigma}_{\text{base}} = [0{,}033;\, 0{,}051;\, 0{,}115]\ \text{mm}^{-1}, \quad
  \hat{\boldsymbol{\sigma}}_{\text{car}} = [0{,}000;\, 0{,}068;\, 1{,}000]
  $$
- **Carotenoid** (Shawkey & Hill [25, 93]) hấp thụ dải xanh lam 400–500 nm và cần nền tán xạ trắng bên dưới $\mathbf{R}_{\text{dermis}}$ để phát sắc vàng/cam (và tự tạo sắc xanh rêu/ô-liu bò sát khi chồng với lượng melanin trung bình):
  $$
  \mathbf{A}_{\text{bio}}(u, v) = \mathbf{R}_{\text{dermis}} \odot \exp\!\Big(-\big(\boldsymbol{\sigma}_a^{\text{epi}}(C_m(v), \beta_m) + C_{\text{car}}(u)\,\kappa_{\text{car}}\,\hat{\boldsymbol{\sigma}}_{\text{car}}\big)\,d_{\text{opt}}\Big)
  $$
- **Iridophores & vảy bò sát:** Ở `LOD0`, thay Fresnel BRDF vi diện bằng **tổng Airy khử răng cưa phổ 2 số hạng** (Belcour & Barla, SIGGRAPH 2017 [27, 100]) có kẹp an toàn $\alpha_{\text{roughness}} \ge 0{,}1$ chống *fireflies* [99], kèm dập nổi pháp tuyến vảy tế bào Voronoi dọc toạ độ xương [101, 102].

---

## 4. Tối ưu 1.000+ Sinh vật trên iGPU: Giải bài toán VAT, Impostor và Mega-Buffer

### 4.1. Bẫy bùng nổ VRAM của VAT / Animated Impostors khi 1.000 cá thể độc bản (`Mô hình ngoài`)

Tính khả thi của kỹ thuật đám đông tiêu chuẩn phụ thuộc hoàn toàn vào việc 1.000 sinh vật dùng chung bao nhiêu kiểu hình cơ thể (xương + lưới):

| Kịch bản tiến hoá | VAT (`1.024` đỉnh × `64` frame × `12 B`) | Animated Hemi-Octahedral Impostors (`8×8 × 12 frame × 64² px`) | Khuyến nghị kiến trúc |
|---|---|---|---|
| **Trường hợp A — Phân cụm Loài (`16–32` nguyên mẫu tĩnh)** *(gần thiết kế cache `morphologyKey` hiện hành)* | **`0,75 MB/loài` → `24,0 MB` (32 loài)** [109] | **`6,0 MB/loài` (nén BC 2 B/px) → `192,0 MB` (32 loài)** [114] | Khả thi với VAT tầm trung + Impostor tĩnh cho đám đông lớn *(đổi lại mất IK bám bậc đá ở xa)* |
| **Trường hợp B — Đột biến Liên tục (`1.000` lưới độc bản 100%)** | **`750,0 MB VRAM`** (và mất gộp lô *InstancedStaticMesh*) | **`23,44 GB` (`24.000 MB` RGBA8)** hoặc **`5,86 GB` (`6.000 MB` nén BC)** — **tràn UMA iGPU** | **Cấm dùng** Animated Impostor / VAT độc bản; chuyển sang **Mega-Buffer + Dynamic Billboard Atlas** |

*(Lưu ý số liệu phần cứng iGPU tham chiếu trong báo cáo ngoài: Intel Iris Xe G7 `96 EU` với LPDDR4x/LPDDR5 `68,2–83,2 GB/s` (con số `1,0 MB L2` trong báo cáo ngoài **chưa xác minh**) và quỹ dữ liệu toàn engine ~`795–970 MB/frame` ở 60 FPS [104], khiến riêng drawing un-LODed `1.000 × 1.600 đỉnh × 48 B` ngốn quá nhiều băng thông đỉnh và `50–150 ms` CPU skinning).*

### 4.2. Kiến trúc Mega-Buffer, SSBO Skinning & Dynamic Billboard Cache (`Suy luận` + `Hợp nhất Anima`)

Khi hỗ trợ quần thể đa hình thái liên tục (hoặc fallback lúc số nguyên mẫu `morphologyKey` vượt ngân sách), lưới `LOD0` (~1.600 đỉnh) được rút gọn lúc sinh bằng QEM bảo toàn thuộc tính (Garland & Heckbert 1997; Hoppe 1999 [119, 120]) sang `LOD1` (~600 đỉnh) và `LOD2` (~180 đỉnh) rồi nối tiếp vào **MegaVBO + MegaIBO (`uint16`)** điều khiển qua **4 lệnh `MultiDrawIndirect`** [29] sinh bởi Compute Frustum Culling:

| Trường dữ liệu đỉnh *(cần rà bỏ `cor_and_flags` nếu không dùng CoR)* | Kiểu | Kích thước | Vai trò |
|---|---|---|---|
| `pos_and_u` | `int16x4` | `8 B` | Toạ độ nghỉ lượng tử hoá theo hộp bao + morphogen $u$ 16-bit |
| `norm_tan_v` | `snorm8x4` | `4 B` | Pháp tuyến bát diện `[-1, 1]²` (2 kênh) + morphogen $v$ 8-bit (1 kênh) + **1 kênh còn lại** — không đủ chỗ cho cả tiếp tuyến xương; tiếp tuyến cần dẫn xuất từ trục xương trong shader hoặc thêm byte (`Suy luận`) |
| *(`cor_and_flags` — tuỳ chọn / đề xuất loại bỏ nếu giữ DQS thuần)* | `float16x3 + uint16` | `8 B` | Độ lệch tâm quay $\mathbf{p}^*_i - \mathbf{v}_i$ cho CoR + cờ vảy/cứng *(bỏ giúp giảm đỉnh từ `32 B` → `24 B`, cần giữ trường `flags` trong `uint` riêng hoặc ghép bit vào kênh thừa)* |
| `bone_indices` | `uint8x4` | `4 B` | 4 chỉ số xương cục bộ (+ `baseBoneOffset` từ SSBO) |
| `bone_weights` | `unorm8x4` | `4 B` | 4 trọng số biến dạng chuẩn hoá |
| `morph_coords` | `unorm16x2` | `4 B` | Toạ độ dọc trục xương & góc chu vi cho vảy Voronoi `LOD0` |

Ở `LOD3 (>90 m)`, thay vì nướng lưới góc nhìn 8×8 gây nổ nhiều GB, cấp phát một **Dynamic Billboard Impostor Atlas** cố định (mỗi cá thể ở xa chỉ giữ **đúng 1 ô** chụp trực diện từ camera, cập nhật xoay vòng *round-robin* `30–50 ô/frame` khi lệch góc `> 10°–12°` hoặc qua `5–6` frame). **Điều chỉnh cho Anima:** báo cáo ngoài chọn Atlas `2048²` (`1.024` ô `64²`, RGBA8×2 — báo cáo ghi `31,25 MB`, tổng `131,2 MB` ở 60 FPS ~`2,48 ms` trên Iris Xe G7 theo `Mô hình ngoài`); với mục tiêu **30 FPS + trần VRAM Low** của Anima Map v2, có thể dùng Atlas `1024²` cho máy RAM hạn hẹp và chia sẻ slot Mega-Buffer theo `morphologyKey` (`MAX_ARCHETYPE_CACHE`). **Lưu ý dung lượng:** `1024²` chỉ chứa `256` ô `64²` (~`8 MiB` RGBA8×2), **ít hơn** ~300 cá thể mà bảng dưới xếp vào `LOD3`; phải thu ô xuống, hoặc chỉ cấp ô cho một phần `LOD3` và vẽ phần còn lại bằng marker xa như renderer hiện tại.

| Tầng LOD & cự ly *(mô hình phân bổ 1.000 cá thể của báo cáo ngoài)* | Số cá thể ước tính | Hình học & Skinning | Tư thế & chuyển động phụ | Độ phức tạp Shading & Bóng đổ |
|---|---|---|---|---|
| **LOD0: Cận cảnh (`0–15 m`)** | ~80 | `1.600` đỉnh (`3.000` tri); **DQS 4 trọng số** *(CoR ngoài mặc định, xem Mục 0.1)* | Tư thế đốt từ `SegmentState`, nội suy giữa tick; nghiêng gốc theo địa hình bằng raycast chỉ-đọc; chuyển động phụ **[Trang trí]** tuỳ chọn. Đặt chân/IK/Verlet: **[Tương lai]** | Melanin–Carotenoid + Airy 2 số hạng + Vảy Voronoi; Shadow Map |
| **LOD1: Trung cảnh (`15–40 m`)** | ~220 | `600` đỉnh (`1.100` tri); **DQS (2 trọng số lớn nhất)** | Tư thế đốt từ `SegmentState`, cập nhật xen kẽ khung chẵn/lẻ (nội suy quat) | Melanin–Carotenoid + phản xạ chuẩn (tắt Voronoi/Airy); bóng bằng `LOD2` |
| **LOD2: Viễn cảnh (`40–90 m`)** | ~400 | `180` đỉnh (`320` tri); **LBS 1 trọng số** | Tư thế đốt từ `SegmentState` ở tần số thấp (`10 Hz`); **0 raycast**, cao độ gốc lấy từ mặt đất frontend | Tính màu sắc tố ở Vertex Shader; blob shadow phẳng |
| **LOD3: Hậu cảnh (`>90 m`)** | ~300 | `4` đỉnh Billboard đọc từ **Dynamic Atlas** (`1024²` chỉ đủ `256` ô) | `0 Hz` khung thường; chỉ dựng lại ô từ tư thế `LOD2` khi tới lượt quét | Đọc `1` mẫu Albedo + Normal/Depth từ Atlas; tắt đổ bóng lưới |

**Ngân sách tam giác — phân bổ trên vượt trần (`Đã kiểm` số học với [`qualityTiers.ts`](../../src/components/Landscape/utils/qualityTiers.ts)).** `80×3.000 + 220×1.100 + 400×320 + 300×2 ≈ 610.600` tam giác, **vượt** trần Low `300_000` (dòng 64) khoảng 2 lần và vượt cả trần High `600_000` (dòng 110). Phân bổ này **phải cắt** trước khi dùng: giới hạn số thân liền ở tầng gần theo tier (B1 §6, "K theo tier"), giảm số đỉnh `LOD0`/`LOD1`, hoặc đẩy nhiều cá thể hơn xuống `LOD2`/marker. Ví dụ minh hoạ (`Suy luận`, chưa đo): ở Low, `40×3.000 + 100×1.100 + 200×320 ≈ 294.000` tam giác, ~660 cá thể còn lại dùng billboard hoặc marker xa. Con số chốt cho từng tier phải đo trên Iris Xe thật.

---

## 5. Lộ trình 4 cột mốc từ Thẻ B1 & danh mục tham khảo

### Lộ trình triển khai có phân tách hợp đồng

1. **Mốc 1 (*Thuần hiển thị, vẽ đúng dữ liệu*):** dùng nguyên cây đốt của `decode_genotype` (không chọn gốc hay lọc nút riêng), `Birth Compute Pass` đánh giá `sdRoundCone + smin` có điều kiện theo cây đốt + **Naive Surface Nets** + trọng số cách ly `dist <= 2`, đóng gói MegaVBO/MegaIBO và biến dạng **DQS** theo `SegmentState` từng tick qua `MultiDrawIndirect` (tôn trọng bộ đệm `morphologyKey`), trong trần tam giác của từng tier.
2. **Mốc 2 (*Bám địa hình và lớp trang trí*):** nghiêng gốc hiển thị theo địa hình $T_{\text{vis\_root}} = T_{\text{phys\_root}} \cdot \Delta T_{\text{terrain\_offset}}$ mà giữ nguyên tư thế tương đối giữa các đốt của backend; chuyển động phụ **[Trang trí]** theo `agent_id`, có công tắc tắt. Serpenoid, mạng CPG nhiều chân, đặt chân và IK kiểu Johansen, đuôi Verlet có động lực riêng **không** thuộc mốc này: chúng là **[Tương lai — cần đổi mô phỏng]**, chờ mô phỏng có chân hoặc gửi thêm dữ liệu (góc khớp, pha CPG).
3. **Mốc 3 (*Yêu cầu cập nhật Contract/ADR nếu ảnh hưởng tiến hoá*):** Tích hợp Gray-Scott RD dị hướng Cotangent Laplace–Beltrami tham số hoá bờ gấp $(g_F, g_{\Delta k})$ trên `LOD0` và Beer–Lambert Melanin–Carotenoid + Airy thin-film + Voronoi; cập nhật [`CREATURE_DEVELOPMENT_CONTRACT.md`](../reference/CREATURE_DEVELOPMENT_CONTRACT.md) và [`ADR-0001-creature-development-lifecycle.md`](../decisions/ADR-0001-creature-development-lifecycle.md) trước khi nối gen sắc tố vào đột biến tiến hoá.
4. **Mốc 4 (*Hiệu năng iGPU thật*):** Kích hoạt QEM `LOD1`/`LOD2`, phân tầng tần suất cập nhật tư thế và raycast, và **Dynamic Billboard Atlas** (`1024²` Low / `2048²` High, round-robin `30–50 ô/frame`; với `1024²` chỉ `256` ô, xem Mục 4.2) kèm đo kiểm trực tiếp trên phần cứng Intel Iris Xe thật thay thế cho số liệu mô hình ngoài.

### Tài liệu tham khảo chọn lọc từ nguồn gốc

> **Ghi chú biên tập:** Nguồn nội bộ chưa công bố liên kết (mục `1` trong báo cáo ngoài gốc) đã được loại bỏ. Tài liệu trực tuyến và bài báo chuyên đề cần đối chiếu lại trước khi nâng thành ADR (đặc biệt `[13]`).

- **[2]** Robust quasi-uniform surface meshing of neuronal morphology (PMC9648403): https://pmc.ncbi.nlm.nih.gov/articles/PMC9648403/
- **[3]** `glslify/glsl-smooth-min`: https://github.com/glslify/glsl-smooth-min
- **[4]** `0fps.net` — *Smooth Voxel Terrain (Part 2)* (Marching Cubes vs Surface Nets): https://0fps.net/2012/07/12/smooth-voxel-terrain-part-2/
- **[5]** Bournemouth MSc Thesis — *Procedural Creature Generation and Animation for Games*: https://nccastaff.bournemouth.ac.uk/jmacey/MastersProject/MSc22/01/ProceduralCreatureGenerationandAnimationforGames.pdf
- **[6]** Dionne & de Lasa (2013) — *Geodesic Voxel Binding for Production Character Meshes*: https://scispace.com/pdf/geodesic-voxel-binding-for-production-character-meshes-3tuxwi8evc.pdf
- **[7]** Kavan et al. (2007/2008) — *Skinning with Dual Quaternions*: https://www.gamedevs.org/uploads/skinning-with-dual-quaternions.pdf
- **[9]** Le & Hodgins (SIGGRAPH 2016) — *Real-time Skeletal Skinning with Optimized Centers of Rotation*: https://studios.disneyresearch.com/wp-content/uploads/2020/04/Real-time-Skeletal-Skinning-with-Optimized-Centers-of-Rotation.pdf
- **[10]** Unity Discussions — bài đăng diễn đàn khẳng định Disney Research sở hữu bằng sáng chế CoR, **không nêu số hiệu** (tuyên bố chưa xác minh): https://discussions.unity.com/t/cor-real-time-skeletal-skinning-with-optimized-centers-of-rotation/733353
- **[11]** Joar Jakobsson / Unity Blog — *Exploring Procedural Design In Rain World*: https://unity.com/blog/exploring-procedural-design-rain-world
- **[12]** Rune Skovbo Johansen (GDC 2009) — *Dynamic Walking*: https://runevision.com/tech/locomotion/Dynamic_Walking_GDC_2009.pdf
- **[13]** Rune Skovbo Johansen (2025) — *Procedural Creature Progress 2021–2024*: https://blog.runevision.com/2025/01/procedural-creature-progress-2021-2024.html
- **[14]** Namura & Nakao (2024) — *A central pattern generator network for simple control of gait*: https://d-nb.info/1367565618/34
- **[15]** Namura & Nakao (2024, `arXiv:2404.17139` PDF): https://arxiv.org/pdf/2404.17139
- **[16]** Rune Skovbo Johansen (MSc Thesis) — *Automated Semi-Procedural Animation for Character Locomotion*: https://runevision.com/thesis/rune_skovbo_johansen_thesis.pdf
- **[17]** Hecker et al. (SIGGRAPH 2008, *Spore*) — *Real-time Motion Retargeting to Highly Varied User-Created Morphologies*: https://www.cse.chalmers.se/edu/year/2011/course/TDA361/Advanced%20Computer%20Graphics/027-hecker%20copy.pdf
- **[18]** Verlet Integration Tutorial (`dt_n / dt_{n-1}`): https://moonjump.com/game-dev-mechanics-verlet-integration-how-it-works/
- **[22]** Greg Turk (SIGGRAPH 1991) — *Generating Textures on Arbitrary Surfaces Using Reaction-Diffusion*: https://faculty.cc.gatech.edu/~turk/my_papers/reaction_diffusion.pdf
- **[23]** ETH IGL — *Laplacian Mesh Optimization* (Cotangent discretization): https://igl.ethz.ch/projects/Laplacian-mesh-processing/Laplacian-mesh-optimization/lmo.pdf
- **[24]** Witkin & Kass (SIGGRAPH 1991) — *Reaction-Diffusion Textures*: https://publications.ri.cmu.edu/storage/publications/pub_files/pub1/witkin_andrew_1991_1/witkin_andrew_1991_1.pdf
- **[25]** Shawkey & Hill — *Carotenoids need structural colours to shine* (PMC1626226): https://pmc.ncbi.nlm.nih.gov/articles/PMC1626226/
- **[26]** Donner & Jensen (SIGGRAPH 2006) — *A Spectral BSSRDF for Shading Human Skin*: http://graphics.ucsd.edu/~henrik/papers/skin_bssrdf/skin_bssrdf.pdf
- **[27]** Belcour & Barla (SIGGRAPH 2017) — *A Practical Extension to Microfacet Theory for the Modeling of Varying Iridescence*: https://belcour.github.io/blog/research/publication/2017/05/01/brdf-thin-film.html
- **[29]** `three.js#28389` — WebGPU `drawIndirect` / `drawIndexedIndirect`: https://github.com/mrdoob/three.js/issues/28389
- **[30]** Karl Sims (SIGGRAPH 1994) — *Evolving Virtual Creatures*: https://www.karlsims.com/papers/siggraph94.pdf
- **[31]** Karl Sims (ALife 1994) — *Evolving 3D Morphology and Behavior by Competition*: https://www.karlsims.com/papers/alife94.pdf
- **[32]** (không phải bài của Karl Sims; tác giả chưa đối chiếu lại) — *A virtual creatures model for studies in artificial evolution*: https://www.researchgate.net/publication/4201451_A_virtual_creatures_model_for_studies_in_artificial_evolution
- **[33]** Zhu et al. (2015) — *High-quality tree structures modelling using local convolution*: https://xqzhu.net/media/15vc.pdf
- **[34]** Polycount Wiki — *Vertex skinning*: http://wiki.polycount.com/wiki/Vertex_skinning
- **[35]** *How The Spore Creature Creator Works*: https://remptongames.com/2022/08/07/how-the-spore-creature-creator-works/
- **[36]** Zhu et al. (2012) — *Analytical solutions for sketch-based convolution surface modeling*: https://xqzhu.net/media/12vc.pdf
- **[37]** Interaction of Fluids with Deformable Solids / Convolution overview: https://matthias-research.github.io/pages/publications/casa2004.pdf
- **[38]** Jin et al. (2009) — Finite-support quartic kernel (`ConvMo`): http://www.cad.zju.edu.cn/home/zhx/papers/ConvMo-final.pdf
- **[39]** Hubert & Cani (2010) — *Convolution Surfaces based on Polygonal Curve Skeletons*: https://inria.hal.science/inria-00429358/file/HubertCani2010_11.pdf
- **[41]** *Convolution Surfaces: Cauchy Kernel* (Lecture Notes): https://graphicscomputing.fr/course/2021_2022/mpri_2-39/lecture/03_modeling/html/content/032_convolution_surfaces_cauchy_kernel/index.html
- **[42]** `superboubek/SQEM`: https://github.com/superboubek/SQEM
- **[43]** Thiery, Guy & Boubekeur (SIGGRAPH Asia 2013) — *Sphere-Meshes*: https://perso.telecom-paristech.fr/boubek/papers/SphereMeshes/SphereMeshes_lowres.pdf
- **[44]** SDF Shape Catalogue (`sdRoundCone`): https://docs.haply.co/inverseSDK/service/commons/sdf-shapes
- **[45]** `CorentinDumery/BMesh`: https://github.com/CorentinDumery/BMesh
- **[46]** Nicholas Bishop — *Blender Skin Modifier Dev Notes*: https://nicholasbishop.net/category/blender/skin-modifier/
- **[48]** Zhu et al. — *Analytical solutions for tree-like structure modelling*: https://xqzhu.net/media/15jvca.pdf
- **[50]** Godot Surface Nets discussion vs Marching Cubes: https://www.reddit.com/r/godot/comments/1j37ult/friendship_ended_with_marching_cubes_now_surface/
- **[51]** *Isosurface* (Marching Cubes & Dual Contouring overview): https://en.wikipedia.org/wiki/Isosurface
- **[52]** Cinevva — *3D Brush Techniques & GPU Voxel Meshing*: https://app.cinevva.com/guides/terrain-sculpting-brushes
- **[53]** *Building Destructible Terrain in C++: Scalar Fields, Meshing*: https://medium.com/@niekmos1/building-destructible-terrain-and-how-i-underestimated-it-c60ebf00ef96
- **[55]** *Occupancy-Based Dual Contouring* (ResearchGate): https://www.researchgate.net/publication/384245782_Occupancy-Based_Dual_Contouring
- **[56]** *Occupancy-Based Dual Contouring* (`arXiv:2409.13418`): https://arxiv.org/html/2409.13418v1
- **[64]** David Belgrod — *Optimized Centers of Rotation*: https://www.dbelgrod.com/project/blend-skinning/
- **[65]** Kavan et al. (TOG 2008) — *Geometric Skinning with Approximate Dual Quaternion Blending*: https://dcgi.fel.cvut.cz/home/zara/papers/KavanEtAll-TOG08.pdf
- **[67]** Le & Hodgins — *Real-time skeletal skinning with optimized centers of rotation* (trang Semantic Scholar của cùng bài [9]; **không** phải nguồn cho cách pha trộn DQS/LBS): https://www.semanticscholar.org/paper/Real-time-skeletal-skinning-with-optimized-centers-Le-Hodgins/efa775a35670f53bfeb023576973b53570b125bb
- **[68]** Vaillant et al. (SIGGRAPH 2013) — *Implicit Skinning*: https://scispace.com/pdf/implicit-skinning-real-time-skin-deformation-with-contact-z4q0rqxdl4.pdf
- **[70]** Rodolphe Vaillant — *Implicit Skinning summary*: https://rodolphe-vaillant.fr/entry/31/implicit-skinning-real-time-skin-deformation-with-contact-modeli
- **[71]** Kavan et al. (I3D 2007 PDF) — *Skinning with Dual Quaternions*: https://users.cs.utah.edu/~ladislav/kavan07skinning/kavan07skinning.pdf
- **[72]** Joar Jakobsson (GDC 2016) — *Animation Bootcamp: 'Rainworld' Animation Process*: https://www.gdcvault.com/play/1023475/Animation-Bootcamp-Rainworld-Animation
- **[74]** *Rain World Full Devlog*: https://candlesign.github.io/Rain-World-Devlog/Full%20devlog
- **[75]** Serpentine Locomotion Dynamics (Hirose curve validation): https://www.researchgate.net/publication/341272677_Analysis_and_Validation_of_Serpentine_Locomotion_Dynamics_of_a_Wheeled_Snake_Robot_Moving_on_Varied_Sloped_Environments
- **[76]** Hirose Snake-Like Robots PDF: https://diving-rov-specialists.com/index-b-documents_htm_files/rov_199-snake-like-robots.pdf
- **[80]** Runevision — *Locomotion System*: https://runevision.com/thesis/
- **[81]** Aristidou & Lasenby (2011) — *FABRIK*: https://www.andreasaristidou.com/publications/papers/FABRIK.pdf
- **[83]** Verlet tentacle / tail angular constraints: https://www.lexaloffle.com/bbs/?tid=154658
- **[86]** Greg Turk PhD Thesis — *Texturing Surfaces Using Reaction-Diffusion*: https://faculty.cc.gatech.edu/~turk/my_papers/turk_thesis_1.pdf
- **[87]** Gray-Scott Reaction-Diffusion equations & Pearson parameters: https://www.mysimulator.uk/content/articles/gray-scott-reaction-diffusion.html
- **[93]** Role of Carotenoids in Skin (PMC6264659): https://pmc.ncbi.nlm.nih.gov/articles/PMC6264659/
- **[99]** Saarland CG — *Iridescence numerical roughness clamping*: https://graphics.cg.uni-saarland.de/competition/cg-2025/original/ekans/iridescence.html
- **[100]** Belcour & Barla (2017) Slides: https://belcour.github.io/blog/slides/2017-brdf-thin-film/slides.html
- **[101]** Reptile scale paradigm: Evo-Devo (PMC2874329): https://pmc.ncbi.nlm.nih.gov/articles/PMC2874329/
- **[102]** Cavalry — *Voronoi Shader*: https://cavalry.studio/docs/nodes/effects/shaders/voronoi-shader/
- **[104]** NanoReview — Intel Iris Xe G7 (96EU): https://nanoreview.net/en/gpu/intel-iris-xe-graphics-g7-96eu
- **[109]** Stoyan3D — *Vertex Animation Texture (VAT)*: https://stoyan3d.wordpress.com/2021/07/23/vertex-animation-texture-vat/
- **[114]** Ryan Brucks — *Hemi-Octahedral Impostors*: https://yummers.dev/hemi-octahedral-impostors.html
- **[119]** Hoppe (SIGGRAPH 1999) — *New quadric metric for simplifying meshes with appearance attributes*: https://hhoppe.com/newqem.pdf
- **[120]** Garland & Heckbert (SIGGRAPH 1997) — *Surface Simplification Using Quadric Error Metrics*: https://www.cs.cmu.edu/~garland/Papers/quadrics.pdf
