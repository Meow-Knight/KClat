# Intent: cửa sổ Settings, hai lựa chọn ngôn ngữ, bản macOS

> Chốt qua phỏng vấn ngày 2026-10-13. Đây là "muốn gì", chưa phải "làm thế nào". Chi tiết kỹ thuật nằm ở
> [`docs/specs/settings-i18n-macos.md`](../specs/settings-i18n-macos.md).

| | |
|---|---|
| **Outcome** | Klat có một **cửa sổ Settings** riêng để chỉnh mọi thứ; tray chỉ còn thao tác nhanh; Klat chạy được trên **macOS**. |
| **Tray** | Chỉ giữ Ready, Pause, Dismiss all, *Settings…*, Quit. |
| **Giao diện Settings** | Bố cục chặt kiểu Settings của hệ điều hành: sidebar trái (General / Language / Hooks / Advanced), mỗi dòng có tên setting, mô tả ngắn và control bên phải, tự đổi sáng/tối theo hệ thống. Thêm chất Klat ở mức vừa phải: mèo nhỏ ở header, màu nhấn lấy từ aura, phần Language cho xem trước một câu thoại mẫu. Phải đẹp, bắt mắt, bố cục chặt chẽ và hợp lý. |
| **Nội dung Settings** | SFW, Hide while sleeping, Start at login, Reduce motion; Hooks (Install/Repair/Uninstall); 3 tham số đang ẩn (khoảng cách giữa hai lần gọi, số lần gọi tối đa, thời gian session bị coi là cũ); mục Advanced chứa Wake test và Sleep. |
| **Ngôn ngữ** | **Platform Language** (English / Tiếng Việt) đổi toàn bộ chữ giao diện: tray, Settings, thông báo. Mặc định English. **Klat Language** (English / Tiếng Việt / Auto) chỉ đổi câu thoại của mèo; Auto random cả hai thứ tiếng. Mặc định Auto. Đổi setting là áp dụng ngay, không cần nút Save. |
| **User** | Người dùng thường: tải `.dmg` về và mở theo guide có ảnh. Dev: tự build từ source theo guide. |
| **Success** | Trên Windows mọi thứ chạy như cũ, cộng thêm cửa sổ Settings và hai lựa chọn ngôn ngữ. Code Rust biên dịch được cho target macOS. Có hai tài liệu: build `.dmg` (cho dev) và cài đặt, mở app khi chưa ký (cho người dùng thường). |
| **Constraint** | `.dmg` miễn phí, không ký, không notarize. Không có Mac lúc làm: lần build đầu trên Mac của chủ dự án là lần chạy thử thật đầu tiên. |
| **Out of scope** | Developer ID / notarize; CI / GitHub Actions; auto-update; ngôn ngữ thứ ba; bản Linux; test trên Mac thật (chủ dự án tự làm sau). |
