# Log Editor Design System (Zed Minimalist Theme)

A developer-centric, minimalist design system engineered for GPUI log viewing and analysis applications, modeled directly on the design language and token schemas of Zed Editor.

---

## 1. Core Principles

1. **Content-as-Hero**: Chrome recedes into the background; log data, timestamps, and severity levels take center stage.
2. **Tonal Elevation over Harsh Borders**: Spatial depth is communicated through subtle, cohesive surface shifts (`background` → `surface_background` → `element_background` → `elevated_surface`), avoiding heavy nested box containers.
3. **Ghost Interactive Elements**: Buttons, rail tabs, and tool toggles are borderless ghost elements by default, illuminating gently with rounded pill highlights (`rounded_md`) upon hover and active interaction.
4. **Micro-Typography & Restrained Accents**: Precise hierarchy (11px micro-labels, 12px UI controls, crisp monospace for data streams). Status indicators use soft, translucent tinted backgrounds with crisp text rather than jarring solid neon blocks.

---

## 2. Three-Layer Token Architecture

```
Layer 1: Primitives (Raw RGBA / Hex)
   ↓
Layer 2: Semantic Tokens (Surfaces, Borders, Text, Icons, Status)
   ↓
Layer 3: Component Tokens (Titlebar, Toolbar, Rail, Canvas Gutter, Inspector, Status Bar)
```

### Layer 2: Semantic Tokens (`ZedTheme`)

| Semantic Token | Value (Zed Dark) | Purpose |
|---|---|---|
| `background` | `#18181b` | Base application window canvas |
| `surface_background` | `#1f1f23` | Grounded surfaces: titlebar, rail, drawer, status bar |
| `editor_background` | `#18181b` | Primary data view (log canvas) |
| `elevated_surface` | `#232328` | Floating popovers, command palette, context modal |
| `element_background` | `#27272a` | Solid input fields, filter badges, pill frames |
| `element_hover` | `#2e2e35` | Hovered log row and button state |
| `element_selected` | `#34343e` | Active row highlight, active segment |
| `ghost_hover` | `#2b2b32` | Soft ghost button hover |
| `ghost_selected` | `#303038` | Active tab / rail icon background |
| `border_variant` | `#27272e` | 1px subtle divider between layout regions |
| `border` | `#383842` | Structural border for floating popovers |
| `border_focused` | `#4f7be8` | Focus ring for inputs and active selections |
| `text` | `#f4f4f5` | Primary high-contrast readability |
| `text_accent` | `#60a5fa` | Search matches, active filter highlights |
| `text_muted` | `#8a8a93` | Metadata, line numbers, column headers |
| `text_placeholder` | `#52525b` | Inactive input prompts |

#### Restrained Semantic Status Badges
| Status | Text Color | Background Tint | Border |
|---|---|---|---|
| **Error / Fatal** | `#f87171` | `rgba(59, 20, 24, 0.8)` | `#5c1d24` |
| **Warning** | `#fbbf24` | `rgba(54, 42, 10, 0.8)` | `#543e0e` |
| **Info** | `#60a5fa` | `rgba(14, 42, 74, 0.8)` | `#1e3a5f` |
| **Success** | `#4ade80` | `rgba(13, 40, 24, 0.8)` | `#184728` |
| **Debug** | `#94a3b8` | `rgba(30, 36, 48, 0.8)` | `#27272e` |

---

## 3. Spatial Layout Blueprint & Boundaries

```
+-------------------------------------------------------------------------------------------------+
| Window Chrome / Titlebar (H: 36px, surface_background, border_b: border_variant)                |
| [Zed Logs] [app.log pill]                                         [⌘K Palette] [Open...]        |
+-------------------------------------------------------------------------------------------------+
| Editor Toolbar (H: 34px, surface_background, border_b: border_variant)                          |
| [Search in logs... ⌘F] [+ Filter] [Level: ERROR ×]           [Density: Compact] [Wrap] [Insp ⌘I] |
+--------+------------------------+-----------------------------------------------+---------------+
| Rail   | Dock Drawer            | Log Canvas (Editor Canvas)                    | Inspector     |
| (40px) | (240px Collapsible)    | Gutter (Line # & Marker) | Time | Lvl | Mod | Msg | (280px Slide) |
| [Logs] | BOOKMARKS & MARKERS    | 0001 ●  12:04:01.120 [ERR] API Timeout err    | Header / Tabs |
| [Mark] | > app.log              | 0002    12:04:01.122 [INF] API Retrying       | Fields        |
| [Mods] |                        | 0003    12:04:01.125 [DBG] DB Pool connected  | Raw           |
|        |                        |                                               | Notes         |
+--------+------------------------+-----------------------------------------------+---------------+
| Status Bar (H: 24px, surface_background, border_t: border_variant)                              |
| Ln 1 · 12,000 events · All Visible · UTF-8                                      | Tail: OFF     |
+-------------------------------------------------------------------------------------------------+
```

### Layout Hierarchy Details
1. **Window Chrome (36px)**:
   - File identifier formatted as a subtle grounded pill (`element_background`, `border_variant`).
   - Borderless ghost action buttons (`⌘K Palette`, `Open...`) lighting up on hover (`ghost_hover`).
2. **Editor Toolbar (34px)**:
   - Compact ghost search trigger (`⌘F`) with focus ring when open (`border_focused`).
   - Filter chips styled as delicate rounded badges (`rounded_md`, `element_background`, `text_accent`) with micro `×` dismiss icons.
   - Right-aligned density and wrap ghost switches.
3. **Activity Rail (40px)** & **Dock Drawer (240px)**:
   - 40px width with 32x32px centered ghost icon buttons. Active tab displays `ghost_selected` with accent icon fill (`icon_accent`).
   - 240px drawer uses micro-typography section headers (`text_xs font_semibold text_muted uppercase`) and indented tree items.
4. **Log Canvas & Gutter**:
   - Column headers: 26px height with `border_variant` bottom line, clear column alignment.
   - Gutter: Line numbers right-aligned in 64px width, muted by default, highlighted on active selection (`gutter_active_line_number`).
   - Clean rows: Borderless, soft hover (`element_hover`), single-line active indicator (`active_line_bg` with left 2px focus bar `border_focused`).
5. **Inspector Panel (280px)**:
   - Slide-out drawer with tabbed segments (`Fields`, `Raw`, `Hex`, `Notes`).
   - Key-value metadata with muted labels and monospace data values.
6. **Floating Overlays (Command Palette & Search)**:
   - Centered floating modal on semi-transparent backdrop.
   - `elevated_surface` (`#232328`), 1px `border` (`#383842`), `rounded_lg`, and soft drop shadow.
