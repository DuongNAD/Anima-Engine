# Danh sách công việc chưa hoàn thiện & Kế hoạch tiếp theo (TODO)

Hệ thống vừa bị khởi động lại (restart) do tài nguyên máy yếu khi chạy toàn bộ dự án Bevy/Tauri. Dưới đây là tình trạng hiện tại và các công việc cần làm tiếp theo để tối ưu hóa và hoàn thiện mô hình Thỏ 3D theo phong cách Papercraft (gấp giấy có góc cạnh và viền đen).

---

## 1. Các công việc đã hoàn thành
- **Tạo Sandbox Độc lập (rabbit-standalone)**: Giúp chạy thử mô hình thỏ trực tiếp bằng HTML/Three.js mà không cần chạy backend Tauri/Rust nặng nề.
- **Sửa lỗi chân thụt thò (Leg Clipping)**: Đã áp dụng công thức co giãn chân động (Dynamic Leg Extension) theo nhịp nhảy `hopY` trên cả bản standalone và component React. Chân không còn bị tách rời hay lún sâu vào thân.
- **Nâng cấp thẩm mỹ Cute**: Đã thêm má hồng, mũi hồng, miệng và mắt bóng có tia sáng phản chiếu (eye glints).
- **Vượt qua kiểm thử**: Toàn bộ 56 frontend tests và quy trình build đều thành công (`npm run test:frontend`, `npm run build`).

---

## 2. Các phần CHƯA HOÀN THIỆN (Cần làm tiếp theo)
Theo yêu cầu tham khảo mô hình 3D mới (phong cách Papercraft/Low-poly gấp giấy):

### Tác vụ 1: Chuyển đổi mô hình sang phong cách Papercraft (Faceted Low-Poly)
- [ ] **Giảm phân đoạn hình học (Low Segment Counts)**: 
  - Chuyển các khối cầu (Sphere), khối con nhộng (Capsule) của thân, đầu, tai, chân thành các khối có số phân đoạn cực thấp (ví dụ: 4 đến 8 phân đoạn) để tạo góc cạnh sắc nét, thô ráp.
- [ ] **Áp dụng Flat Shading**: 
  - Bật thuộc tính `flatShading: true` trên tất cả các chất liệu (`MeshStandardMaterial`) để ánh sáng phản chiếu phẳng theo từng mặt đa giác, không làm mịn bề mặt.
- [ ] **Thêm viền đen sắc nét (Black Outlines)**:
  - Sử dụng `THREE.EdgesGeometry` kết hợp với `THREE.LineSegments` vẽ đè lên mỗi bộ phận để tạo viền đen chạy dọc theo các cạnh sắc của khối đa giác, mô phỏng đúng phong cách mô hình giấy gấp.

### Tác vụ 2: Điều chỉnh dáng đứng (Sitting Posture) & Tỷ lệ
- [ ] **Dáng đứng co chân**: Điều chỉnh tư thế mặc định của thỏ giống dáng ngồi khép chân trong ảnh tham khảo.
- [ ] **Tai nhọn góc cạnh**: Thiết kế lại tai thỏ sử dụng hình nón hoặc hình trụ 4 cạnh vuốt nhọn ở đỉnh.

### Tác vụ 3: Đồng bộ động học hoạt ảnh
- [ ] Cập nhật hoạt ảnh nhảy (hopping) và nhai (chewing) để hoạt động mượt mà với cấu trúc hình học Papercraft góc cạnh mới mà không làm mất đi tính tương tác của các thanh trượt điều khiển.

---

## 3. Hướng dẫn chạy nhẹ nhàng cho máy yếu (Tránh treo máy)
Để không làm quá tải CPU/RAM dẫn đến việc máy phải tự khởi động lại:
1. **KHÔNG chạy Tauri backend** (`npm run tauri dev` hoặc các lệnh Cargo).
2. **Chỉ chạy server tĩnh siêu nhẹ** phục vụ thư mục độc lập:
   ```bash
   # Đã được khởi chạy ở cổng 8000
   py -m http.server 8000
   ```
3. Mở trình duyệt truy cập: `http://localhost:8000/` để xem và tương tác trực tiếp với mô hình thỏ mà không tốn tài nguyên hệ thống.
