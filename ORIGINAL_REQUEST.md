# Original User Request

## Follow-up — 2026-08-16T04:54:37Z

Nâng cấp và tối ưu hóa toàn diện bản đồ 3D Map 2 của Anima Engine theo các mô hình khoa học và kiến trúc tạo thế giới mã nguồn mở hàng đầu thế giới (WorldEngine, worldgen Rust, mapgen4 Red Blob Games, TerraForge3D, Whittaker Biome Classification), tích hợp các quy luật địa chất kiến tạo mảng (Plate Tectonics), xói mòn thủy lực (Hydraulic & Thermal Erosion), mô hình khí hậu hoàn lưu gió & bóng mưa (Orographic Rain Shadow), mạng lưới lưu vực sông tự nhiên (Dendritic Hydrological Routing) và kết xuất thời gian thực Three.js tối ưu 60 FPS.

Working directory: `E:\Project\Anima-Engine`
Integrity mode: development

## Requirements

### R1. Tectonic Geology & Hydraulic Erosion Pipeline (Lấy cảm hứng từ WorldEngine & TerraForge3D)
- Mô hình hóa địa hình dựa trên sự tương tác của các mảng kiến tạo (Tectonic Plates): Dãy núi nếp gấp sừng sững hình thành tại ranh giới va chạm mảng, thung lũng tách giãn (rift valleys) và đồng bằng lục địa vững chắc ($\ge 85\%$ diện tích đất liền).
- Áp dụng thuật toán xói mòn thủy lực và nhiệt (Hydraulic & Thermal Erosion):
  - Dòng chảy xói mòn sườn dốc tạo các khe rãnh tự nhiên (gullies & ridges).
  - Vách đá sạt lở tạo các bãi bồi sỏi đá chân núi với góc dốc tự nhiên (talus angle of repose).

### R2. Atmospheric Climate & Orographic Rain Shadow (Lấy cảm hứng từ mapgen4 & World Orogen)
- Hệ thống mô phỏng khí hậu hoàn chỉnh 2 trục (Nhiệt độ & Độ ẩm):
  - **Nhiệt độ**: Suy giảm theo độ cao (elevation lapse rate $-6.5^\circ\text{C} / \text{km}$).
  - **Hoàn lưu gió & Hiệu ứng bóng mưa (Rain Shadow Effect)**: Gió thịnh hành mang theo hơi ẩm từ biển, gặp dãy núi cao chắn lại sẽ trút mưa xối xả ở sườn đón gió (tạo rừng mưa nhiệt đới và rừng ôn đới trù phú), đồng thời tạo vùng khuất gió khô cằn ở sườn bên kia (tạo savan bán hoang mạc và cồn cát sa mạc).

### R3. Dendritic River Basin & Hydrological Network (Mạng Lưới Thủy Văn Tự Nhiên)
- Mô phỏng dòng chảy trọng lực (Downstream Flow Routing): Nước tích tụ từ đỉnh núi cao chảy xuống thung lũng tạo thành mạng lưới sông suối phân nhánh hình cây (dendritic rivers).
- Sông chảy uốn lượn tự nhiên đổ về hồ nước trung tâm hoặc vịnh biển, bờ nước thoai thoải với bãi cát/sỏi ướt, không có hiện tượng nước phẳng dìm ngập vô cớ hay sông chảy ngược dốc.

### R4. Whittaker Ecological Biome Classification & Carrying Capacity
- Phân loại 8 vùng sinh thái theo sơ đồ Whittaker (Nhiệt độ vs Độ ẩm):
  - Chuyển tiếp mượt mà giữa các biome (ecotones) qua kỹ thuật softmax texture splatting.
  - Cân bằng sức chứa sinh thái (Carrying capacity): Trảng cỏ thoáng đãng rộng lớn ($\ge 65\%$ diện tích), cây cối phân cụm hữu cơ (groves), và quần xã động vật phân bổ theo đàn nhỏ tự nhiên.

### R5. High-Performance Real-Time WebGL Rendering & Dual Integration
- Tối ưu hóa GPU Instancing, ACESFilmic Tone Mapping, tán xạ khí quyển Rayleigh & Mie, sóng Gerstner và bọt nước bờ hồ.
- Giữ vững cơ chế truy cập kép: trang độc lập `map2.html` và nút chuyển đổi nhanh trên Dashboard `App.tsx`, đảm bảo 0 lỗi build/lint và 60 FPS mượt mà.

## Acceptance Criteria

### Scientific Coherence & Physical Realism
- [ ] Địa hình thể hiện rõ rệt các rặng núi uốn nếp kiến tạo, sườn xói mòn và thung lũng tự nhiên.
- [ ] Phân bổ biome tuân thủ quy luật khí quyển (sườn đón gió ẩm ướt trù phú, sườn khuất gió khô cằn sa mạc).
- [ ] Mạng lưới sông suối chảy đúng chiều trọng lực từ cao xuống thấp đổ vào lòng hồ.
- [ ] Cảnh quan thoáng đãng, cân bằng sinh thái, không bị ngập nước chia cắt hay nghẹt thở vì cây cối.

### Performance & Build Quality
- [ ] `npm run build` và `npm run lint` hoàn thành với 0 lỗi.
- [ ] Toàn bộ test suites frontend và map2 tests PASS 100%.
- [ ] Chụp ảnh kiểm thử trực tiếp xác nhận bản đồ đạt độ chân thực cao chuẩn mã nguồn mở quốc tế.
