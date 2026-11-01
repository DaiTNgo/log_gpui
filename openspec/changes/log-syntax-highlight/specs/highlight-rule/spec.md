## ADDED Requirements

### Requirement: Highlight Rule và Marker che Syntax Color
Một Marker hay Highlight Rule SHALL che Syntax Color ở đúng khoảng ký tự hoặc cell nó phủ, theo thứ tự ưu tiên: Marker, Highlight Rule, Search Phrase, Syntax Color, màu mặc định.

#### Scenario: Dòng có Highlight Rule
- **WHEN** một Highlight Rule tô màu một cột của dòng và cell Message của dòng có token được tô
- **THEN** cột được rule tô hiện bằng màu của rule, cell Message vẫn giữ Syntax Color

#### Scenario: Marker trên một đoạn chữ
- **WHEN** một Marker phủ một đoạn chữ đang có màu token
- **THEN** đoạn đó hiện bằng màu Marker và màu chữ của Marker, không phải màu token
