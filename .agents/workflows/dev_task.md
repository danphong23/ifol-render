---
description: Quy trình phát triển tiêu chuẩn cho dự án ifol-render. Sử dụng khi bắt đầu bất kỳ công việc phát triển nào (thêm tính năng, sửa lỗi, refactor).
---

# /dev-task — Quy trình phát triển từng bước

Quy trình bắt buộc cho mọi công việc phát triển trong dự án ifol-render. Mỗi task phải nhỏ, độc lập, và test được.

## Bước 1: Phân tích yêu cầu
- Đọc yêu cầu của người dùng.
- Đọc các Rule liên quan trong `.agents/rules/` và `.agents/design-system/`.
- Nghiên cứu codebase hiện tại để hiểu context.

## Bước 2: Tạo danh sách Tasks
- Chia nhỏ yêu cầu thành danh sách các task độc lập.
- Mỗi task phải đủ nhỏ để có thể test riêng lẻ.
- Mỗi task phải có tiêu chí hoàn thành (acceptance criteria) rõ ràng.
- Trình danh sách cho người dùng duyệt trước khi bắt đầu.
- Format:
```
## Task 1: [Tên task]
- Mô tả: [Làm gì]
- Test: [Cách kiểm tra task này đã hoàn thành]
- Phụ thuộc: [Task nào cần hoàn thành trước, nếu có]
```

## Bước 3: Chờ người dùng phê duyệt danh sách Tasks
- **DỪNG LẠI** và chờ người dùng review danh sách.
- Người dùng có thể yêu cầu thêm/bớt/sửa task.
- Chỉ bắt đầu khi người dùng xác nhận "OK" hoặc "bắt đầu".

## Bước 4: Thực hiện từng Task
Với mỗi task trong danh sách (theo thứ tự):

### 4a. Thực hiện
- Code theo đúng các Rule đã định nghĩa.
- Tuân thủ Design System.

### 4b. Tự kiểm tra
- Chạy build/lint nếu có thể.
- Kiểm tra code có vi phạm Rule nào không.

### 4c. Báo cáo cho người dùng
- Liệt kê các file đã thay đổi/tạo mới.
- Mô tả ngắn gọn những gì đã làm.
- Cung cấp test case (TC) để người dùng tự test.
- Format báo cáo:
```
### ✅ Task N hoàn thành
**Đã thay đổi:**
- file_a.rs: [mô tả thay đổi]
- file_b.svelte: [mô tả thay đổi]

**Test case:**
- TC1: [Mô tả bước test] → Kết quả mong đợi: [...]
- TC2: [Mô tả bước test] → Kết quả mong đợi: [...]
```

### 4d. Chờ người dùng xác nhận PASS
- **DỪNG LẠI** và chờ người dùng test.
- Nếu người dùng báo FAIL: Sửa lỗi → Quay lại 4c.
- Nếu người dùng báo PASS: Chuyển sang 4e.

### 4e. Commit
- `git add` các file liên quan.
- `git commit` với message mô tả rõ ràng.
- Chuyển sang Task tiếp theo (quay lại 4a).

## Bước 5: Hoàn tất
- Khi tất cả task đã PASS, tổng hợp lại toàn bộ thay đổi.
- `git push` lên remote.
- Tạo walkthrough tổng kết nếu cần.
