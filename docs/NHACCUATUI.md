# Ghi chú tích hợp NhacCuaTui

Bản bàn giao ban đầu khảo sát API web beta cũ và biến thể đường dẫn mobile
`/v1`. Hồ sơ đó là đầu vào khảo sát. Kiểm tra lại adapter của dự án YT Music Lite
cho thấy còn một đường khách tại `graph.nct.vn/api/v1`. Từ Windows ngày 18/11/2026,
các phép thử dùng yêu cầu HTTPS thông thường và cho kết quả sau.

| Chức năng | Kết quả đã đối chiếu |
|---|---|
| Tìm bài | Trả `code = 0`, tên, nghệ sĩ, ảnh và thời lượng cho “Nơi Này Có Anh” và “Một Bước Yêu Vạn Dặm Đau”. |
| Chi tiết và phát | Trả luồng MP3 với quyền khách rõ ràng. CDN gửi dữ liệu âm thanh cho bài mẫu. URL được tra lại khi bắt đầu phát. |
| Bài tương tự | Trả danh sách theo bài nguồn. Đây là gợi ý từ NhacCuaTui. |
| Trang chủ | Trả playlist đề xuất và bài trong playlist. YoZiNu hiển thị bìa của các bài có quyền khách. |

Nhạc NCT dùng mã `nct:` và hàng đợi chung. Bài chỉ dành cho VIP hoặc yêu cầu
chế độ phát khác do NCT quản lý được xử lý theo phản hồi của dịch vụ. Phạm vi
Rev0.1 gồm nghe nhạc khách. Đăng nhập NCT, tải xuống, lời bài hát NCT, lượt thích
và biên tập playlist trên máy chủ NCT nằm ngoài phạm vi phiên bản này.

Tệp Android `NCT_8.5.04_297.xapk` được lưu trong tầng tham chiếu cùng bàn giao gốc.
Manifest ghi `ht.nct`, phiên bản `8.5.04`, build `297`. SHA-256 của tệp
là `21cc1a4de6bdc85df0056b5bbb1bbc43a9f98e0db71754a7e980bb63c6ed73c2`.
