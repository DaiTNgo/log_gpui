## ADDED Requirements

### Requirement: Inspector chỉ hiện text lưu trữ
Inspector SHALL chỉ hiện text lưu trữ của dòng đang chọn, đúng từng ký tự, kèm các Continuation Line của dòng đó khi dòng là Parent Line. Inspector SHALL NOT có tab Fields hay Hex.

#### Scenario: Dòng có stack trace
- **WHEN** người dùng chọn một dòng `ERROR` có 4 dòng stack trace phía sau
- **THEN** Inspector hiện dòng `ERROR` và 4 dòng stack trace, đúng như trong file

#### Scenario: Không có tab
- **WHEN** Inspector đang mở
- **THEN** không có tab Fields, Raw hay Hex nào

### Requirement: Inspector ở cả hai view và cả hai khung
Inspector SHALL hiện ở cả Raw View và Parsed View, và SHALL theo dòng được chọn gần nhất, dù dòng đó được chọn ở view hay ở Focus Pane.

#### Scenario: Raw View
- **WHEN** view là Raw View và người dùng chọn dòng 42 của `api.log`
- **THEN** Inspector hiện dòng 42 của `api.log`

#### Scenario: Chọn trong Focus Pane
- **WHEN** người dùng bấm một dòng trong Focus Pane
- **THEN** Inspector hiện dòng đó
