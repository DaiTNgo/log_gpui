## MODIFIED Requirements

### Requirement: Marker được nhớ
Hệ thống SHALL tự lưu Marker theo Generation và số dòng của từng file và khôi phục khi mở lại file hoặc khởi động lại ứng dụng. Sửa nội dung một dòng scratch SHALL làm mất mọi Marker của dòng đó. Dòng rời Live Window SHALL làm mất mọi Marker của dòng đó. Marker của Remote File SHALL chỉ được khôi phục khi dòng có cùng Generation và số dòng còn được giữ sau khi nạp lại.

#### Scenario: Khởi động lại
- **WHEN** người dùng tô ba đoạn trong `api.log`, thoát rồi mở lại ứng dụng
- **THEN** ba Marker vẫn ở đúng chỗ

#### Scenario: Sửa dòng scratch
- **WHEN** dòng 10 của `untitled.log` có Marker và người dùng sửa dòng đó
- **THEN** sau khi lưu, dòng 10 không còn Marker nào

#### Scenario: Hai dòng 1 khác Generation
- **WHEN** dòng 1 của Generation đầu có Marker và file xoay vòng, Generation mới cũng có dòng 1
- **THEN** chỉ dòng 1 của Generation đầu có Marker
