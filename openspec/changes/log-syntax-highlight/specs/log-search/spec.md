## ADDED Requirements

### Requirement: Match vẽ trên Syntax Color
Search Phrase và Current Match SHALL được vẽ che lên Syntax Color ở đúng khoảng ký tự chúng phủ. Tắt Search Highlight SHALL NOT tắt Syntax Color, và tắt Syntax Color SHALL NOT đổi Match nào được tìm thấy hay màu của Phrase.

#### Scenario: Phrase trùng một token
- **WHEN** Search cho `ERROR` và dòng có từ `ERROR` đang được tô màu level
- **THEN** `ERROR` hiện bằng màu của Phrase, phần còn lại của dòng giữ Syntax Color

#### Scenario: Tắt Search Highlight
- **WHEN** người dùng tắt Search Highlight trong Focus Pane
- **THEN** các Match không còn được tô, Syntax Color vẫn còn
