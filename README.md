# YouTube Metadata Analyzer (Rust & yt-dlp)

Ứng dụng desktop cho Windows siêu nhẹ, giao diện Dark Mode Glassmorphism cao cấp, khai thác toàn diện siêu dữ liệu (metadata) của YouTube thông qua `yt-dlp` và `Rust` (Tauri v2).

## Tính năng nổi bật

1. **Thao tác đơn giản 1 chạm**:
   - Dán URL từ clipboard hoặc dùng nút "Dán".
   - Bấm **"Phân tích"** (hoặc gõ Enter) để tự động bóc tách toàn bộ thông tin.
   - Hỗ trợ nút "Dùng link mẫu" để test nhanh ngay lập tức.
2. **Tổng quan video (Overview)**:
   - Ảnh bìa Thumbnail chất lượng cao, nhãn thời lượng, nhãn Live/VOD.
   - Tiêu đề, Tên kênh, Số lượng người đăng ký (subscribers).
   - Video ID, Channel ID, Uploader ID, Giấy phép bản quyền, Giới hạn tuổi.
   - Ngày đăng UTC và giờ địa phương, Timestamp chính xác, Ngày phát hành (Release date).
   - Trạng thái trực tiếp (Live status, Was live).
   - Thư viện ảnh Thumbnail các độ phân giải khác nhau (kèm nút sao chép link ảnh).
3. **Số liệu & Tương tác (Stats & Metrics)**:
   - Thẻ thống kê: Lượt xem (Views), Lượt thích (Likes), Bình luận (Comments).
   - Tỷ lệ tương tác ước tính (Likes / Views ratio).
   - Quy đổi thời lượng ra giây và phút, độ phân giải tối đa, tốc độ khung hình (FPS).
4. **Thẻ từ khóa SEO & Thể loại (Tags & Categories)**:
   - Danh sách thể loại (Categories).
   - Danh sách thẻ Tags trực quan: Bấm vào tag bất kỳ để sao chép tag đó, hoặc bấm "Sao chép tất cả tags" (ngăn cách bằng dấu phẩy) để dán ngay vào trình quản lý video YouTube Studio.
5. **Bảng định dạng & Luồng dữ liệu (Formats & Streams)**:
   - Lọc nhanh: Tất cả, Video + Audio, Chỉ Video, Chỉ Audio.
   - Chi tiết: Format itag ID, định dạng (MP4, WEBM, M4A, OPUS), độ phân giải (4K, 2K, 1080p, 720p...), FPS, Codecs (AV1, VP9, AVC1, Opus), Bitrate (kbps), dung lượng xấp xỉ (MB/GB).
   - Nút "Copy URL" để lấy link stream phát trực tiếp.
6. **Mốc chương (Chapters)**:
   - Danh sách các chương mục theo mốc thời gian và tiêu đề.
   - Nút sao chép từng mốc chương hoặc sao chép toàn bộ danh sách chương.
7. **Nội dung mô tả (Description)**:
   - Khung đọc mô tả đầy đủ với định dạng văn bản chuẩn, có nút sao chép.
8. **Dữ liệu JSON Gốc (Raw JSON)**:
   - Hiển thị 100% cấu trúc JSON mà `yt-dlp` trả về.
   - Nút sao chép toàn bộ JSON.
9. **Tiện ích & Xuất file**:
   - **Xuất JSON**: Tải tệp `.json` đầy đủ về máy.
   - **Sao chép tóm tắt**: Xuất định dạng chuỗi chuẩn:
     `TITLE: ... | CHANNEL: ... | VIDEO ID: ... | UPLOAD DATE UTC: ... | TIMESTAMP: ... | DURATION: ... | VIEWS: ... | LIKES: ... | COMMENTS: ... | CATEGORY: ... | TAGS: ...`
   - **Lịch sử gần đây**: Lưu tự động 20 video phân tích gần nhất, có thể xem lại bất cứ lúc nào.
   - **Tùy chọn Cookies**: Hỗ trợ đọc cookies từ Chrome, Edge, Firefox, Brave nếu cần phân tích video yêu cầu đăng nhập.

## Cách chạy ứng dụng

- **Cách 1**: Nhấp đúp chuột vào file `Chay_App.bat` tại thư mục `D:\YouTubeMetadataAnalyzer`.
- **Cách 2**: Chạy trực tiếp file thực thi tại:
  `D:\YouTubeMetadataAnalyzer\src-tauri\target\release\app.exe`
- **Cách 3 (Dev)**:
  Mở terminal tại `D:\YouTubeMetadataAnalyzer\src-tauri` và gõ:
  ```bash
  cargo run --release
  ```
