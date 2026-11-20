# Lịch sử YoZiNu

## Rev0.6.2 — 20/11/2026

- Playlist tổng hợp có nút **Xóa tất cả**, dọn mọi bài đã lưu và ghi playlist rỗng lên máy. Các nút Nhập/Xuất liên kết vẫn giữ nguyên.
- Feather Lite bỏ dải thông số RAM, nút RAM và nút **Dọn bộ đệm ngay**. Bộ đệm ảnh bìa nhỏ vẫn chạy, nay tự dọn sau mỗi 5 phút dùng bố cục; rời bố cục sẽ đặt lại bộ đếm.
- Bỏ mục **Explore** khỏi thanh bên, các bố cục, danh sách trang mở đầu và bảng lệnh. Nút Home đổi thành **YouTube** và mở thẳng trang Home của YouTube.
- YouTube Home bỏ hai khối quảng cáo chung “Top tunes right now / Today's hits” và “Fresh tunes / New & Now”, kể cả khi đọc từ bản lưu tạm.

## Rev0.6.1 — 20/11/2026

- Feather Lite có hàng điều hướng riêng với **Trang chủ**, **Khám phá**, Zing MP3, NhacCuaTui, **Playlist tổng hợp** và **Cài đặt**. Các nút Sắp phát, Lời và RAM vẫn nằm ở hàng riêng bên dưới. Bộ phát, hàng đợi và ba cơ chế tiết kiệm RAM không đổi.
- Mọi yêu cầu YouTube Music gửi `gl=VN`, còn bảng xếp hạng chọn `VN` thay vì `US`. `hl` giữ `en` để bộ phân tích dữ liệu của ứng dụng vẫn nhận đúng tên mục và chip; giao diện Tiếng Việt của YoZiNu không đổi.

## Rev0.6 — 20/11/2026

- Thêm bốn bố cục, đặt sau sáu bố cục của Rev0.5 trong **Cài đặt → Giao diện → Bố cục**: Aurora Stage, Studio Console, Velvet Gallery, Feather Lite. Tổng cộng 11 lựa chọn; Giao diện gốc cùng toàn bộ theme và bảng màu cũ giữ nguyên.
- **Aurora Stage**: ánh sáng nền dịu đổi tông theo ảnh bìa bài đang phát, thanh bên chia nhóm (Khám phá, Bảng xếp hạng ba nguồn, Playlist tổng hợp, nguồn nhạc), thẻ **Đề xuất hôm nay**, ngăn phải chuyển giữa Sắp phát và Lời bài hát.
- **Studio Console**: bố cục trạm ba cột, bảng kho nhạc nhiều cột, cột phải cố định **Bảng Đang Phát** (thiết bị ra, bộ đệm, tốc độ, mức tín hiệu, lời đồng bộ, Sắp phát) và dải sóng tiến trình bấm để tua ở đáy.
- **Velvet Gallery**: tông ấm, bỏ thanh bên cố định, lưới ảnh bìa bốn cột, ngăn trượt **Phòng nghe** và thanh phát dạng dock nổi.
- **Feather Lite**: bố cục phẳng, dòng 48 điểm, bìa 36 điểm, không hiệu ứng nặng. Ba cơ chế tiết kiệm RAM chạy thật: ảnh bìa tải đúng cỡ hiển thị (tối đa 80 điểm ảnh), chỉ tải cho phần đang nhìn thấy, và bộ đệm LRU 48 ảnh tự giải phóng, kèm dải thông số và nút **Dọn bộ đệm ngay**.
- Bốn bố cục mới theo chế độ Sáng/Tối của Cài đặt; ảnh lời bài hát trên nền sáng đặt trong khung tối để giữ đúng màu lời.

## Rev0.5 — 19/11/2026

- Thêm sáu bố cục Windows trong **Cài đặt → Giao diện → Bố cục**. Người dùng có thể chọn Bộ sưu tập bìa, Danh sách liền mạch, Bàn điều khiển, Khám phá qua ảnh bìa, Kho nhạc, Phòng nghe hoặc Giao diện gốc.
- Dùng chung bộ phát, hàng đợi, Playlist tổng hợp và bộ nhớ đệm ảnh bìa. Chọn bố cục chỉ đổi thành phần giao diện đang hiển thị và lưu tùy chọn vào hồ sơ hiện tại.
- Ở các bố cục mới, tìm kiếm có hàng riêng. Sắp phát có vùng riêng, xem trước bài kế tiếp hoặc mở danh sách đầy đủ dưới thanh tìm ở cửa sổ hẹp.
- Kho nhạc có bảng bài hát theo cột khi đủ chiều rộng. Phòng nghe có màn hình Đang phát gồm ảnh bìa, điều khiển, lời bài hát và Sắp phát.

## Rev0.4.1 — 19/11/2026

- Tách riêng vùng của ô tìm kiếm và bộ chọn nguồn, căn cùng chiều cao và chừa khoảng cách giữa hai nút.
- Khi cửa sổ hẹp, chuyển bộ chọn nguồn sang biểu tượng. Menu vẫn có đủ bốn lựa chọn.

## Rev0.4 — 19/11/2026

- Dùng chung ô tìm kiếm có gợi ý ở đầu cửa sổ, kèm bộ chọn nguồn. Tìm **Tất cả** trả về ba nhóm kết quả.
- Thêm Tiếng Việt trong Cài đặt → Chung, cập nhật Giới thiệu và dọn các mục Kết nối, Nhập dữ liệu, Mạng, Phím tắt, Cập nhật khỏi Cài đặt.
- Nhập và xuất danh sách liên kết bài hát. Tệp đơn giản để lưu hoặc chuyển giữa các máy.
- Thêm liên kết đăng nhập chính thức cho Zing MP3 và NhacCuaTui. YoZiNu tiếp tục phát từ hai nguồn theo quyền khách.
- Kho mã công khai tại [datbi201/YoZiNu](https://github.com/datbi201/YoZiNu).

## Rev0.3 — 19/11/2026

- Làm gọn thanh bên theo ảnh góp ý và thêm phạm vi tìm kiếm **Tất cả** trong Playlist tổng hợp.
- Mở và cập nhật hàng đợi ngay khi thêm bài vào playlist. Bổ sung nội dung Khám phá và bảng xếp hạng cho hai trang nguồn.
- Bật lời đồng bộ theo từ của Zing và tra LRCLib cho bản thu khớp từ NhacCuaTui hoặc khi Zing thiếu lời.

## Rev0.2 — 19/11/2026

- Bổ sung playlist tổng hợp lưu trên máy cho YouTube Music, Zing MP3 và NhacCuaTui. Tìm bài theo nguồn và thêm trực tiếp từ kết quả.
- Hiển thị ký hiệu nguồn, lọc theo nguồn hoặc tên bài/nghệ sĩ, đổi thứ tự, bỏ bài và phát theo danh sách đang xem.

## Rev0.1 — 18/11/2026

- Dùng cấu trúc ba tầng dưới `D:\My Projects\YoZiNu`. Nhập bản YouTube Music và Zing MP3 đã kiểm chứng từ ổ G vào Git cục bộ.
- Bổ sung nguồn NhacCuaTui dùng dữ liệu khách, gồm trang chủ theo playlist đề xuất, tìm kiếm, phát và tự chuyển bài từ danh sách bài tương tự của NCT.
- Dùng chung giao diện duyệt bài, Sắp phát và trình giải mã MP3 giữa Zing MP3 và NhacCuaTui.
