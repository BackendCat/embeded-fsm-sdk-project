# FSM Studio — Design System & Style Guide

**Document ID:** FSM-SPEC-STYLE
**Version:** 1.0.0
**Status:** Normative Draft
**Applies to:** Web IDE · VS Code WebView panels

---

# 1. Design Principles

## 1.1 Core Values

| Value | Expression |
|---|---|
| **Precision** | Tight grids, consistent spacing, no visual noise. |
| **Legibility** | Information density without clutter. Monospace where it matters. |
| **Hierarchy** | Clear visual distinction between structure levels. |
| **Context-awareness** | Adapts to VS Code theme automatically. |
| **Industrial trust** | Not a consumer app. Calm, reliable, professional. |

## 1.2 Design Anti-Patterns (Forbidden)

- Decorative gradients not conveying state.
- Rounded corners > 12px on any container.
- Drop shadows heavier than defined in §5.5.
- Animations > 400ms on functional UI (diagrams may use up to 600ms).
- Unsolicited notifications (toast only for user-triggered actions or errors).
- Colour as the sole differentiator of critical state (always pair with icon/label).
- Font sizes below 11px.

---

# 2. Colour System

## 2.1 Dark Theme (Primary)

Dark is the default theme. Embedded developers overwhelmingly prefer dark environments.

### Background Hierarchy

| Token | Hex | Usage |
|---|---|---|
| `--bg-base` | `#0D1117` | Application root background. |
| `--bg-surface` | `#161B22` | Panels, cards, sidebar. |
| `--bg-elevated` | `#21262D` | Inputs, dropdowns, raised surfaces. |
| `--bg-overlay` | `#2D333B` | Tooltips, context menus, modals. |
| `--bg-subtle` | `#1C2128` | Hover background on list items. |

### Text Hierarchy

| Token | Hex | Usage |
|---|---|---|
| `--text-primary` | `#E6EDF3` | Main content, labels. |
| `--text-secondary` | `#8B949E` | Muted descriptions, annotations. |
| `--text-tertiary` | `#484F58` | Placeholders, disabled labels, region names. |
| `--text-inverse` | `#0D1117` | Text on accent-filled surfaces. |
| `--text-link` | `#58A6FF` | Clickable diagnostic codes, links. |

### Border

| Token | Hex | Usage |
|---|---|---|
| `--color-border` | `#30363D` | Panel borders, dividers. |
| `--color-border-muted` | `#21262D` | Subtle separators. |
| `--color-border-strong` | `#484F58` | Input focus ring (unfocused). |

### Accent (Electric Blue)

| Token | Hex | Usage |
|---|---|---|
| `--accent-primary` | `#2F81F7` | Primary buttons, active state borders, links. |
| `--accent-hover` | `#4D94FF` | Button hover. |
| `--accent-muted` | `#2F81F71A` | 10% opacity — hover backgrounds, subtle fills. |
| `--accent-strong` | `#1A6EDB` | Active/pressed state. |

### Semantic

| Token | Hex | Usage |
|---|---|---|
| `--color-success` | `#3FB950` | Final states, success status, positive values. |
| `--color-success-muted` | `#3FB9501A` | Success background. |
| `--color-warning` | `#D29922` | Warnings, timed transitions, recently exited state. |
| `--color-warning-muted` | `#D299221A` | Warning background. |
| `--color-error` | `#F85149` | Errors, invalid states, critical badges. |
| `--color-error-muted` | `#F851491A` | Error background. |
| `--color-info` | `#79C0FF` | Info diagnostics, hints. |
| `--color-history` | `#8957E5` | History pseudo-states. |
| `--color-choice` | `#E3B341` | Choice/junction pseudo-states. |

## 2.2 Light Theme

Triggered by system preference or explicit toggle.

| Token | Hex |
|---|---|
| `--bg-base` | `#FFFFFF` |
| `--bg-surface` | `#F6F8FA` |
| `--bg-elevated` | `#EAEEF2` |
| `--bg-overlay` | `#FFFFFF` |
| `--bg-subtle` | `#F3F4F6` |
| `--text-primary` | `#1F2328` |
| `--text-secondary` | `#656D76` |
| `--text-tertiary` | `#9198A1` |
| `--text-inverse` | `#FFFFFF` |
| `--text-link` | `#0969DA` |
| `--color-border` | `#D0D7DE` |
| `--color-border-muted` | `#EAEEF2` |
| `--color-border-strong` | `#9198A1` |
| `--accent-primary` | `#0969DA` |
| `--accent-hover` | `#0860CA` |
| `--accent-muted` | `#0969DA1A` |
| `--accent-strong` | `#0550AE` |
| `--color-success` | `#1A7F37` |
| `--color-warning` | `#9A6700` |
| `--color-error` | `#CF222E` |
| `--color-history` | `#6639BA` |
| `--color-choice` | `#9A6700` |

## 2.3 VS Code WebView Token Mapping

In VS Code WebView panels, all custom tokens MUST map to VS Code CSS variables to
respect the user's installed theme. The mapping is applied via CSS custom property
override injected by the extension host:

```css
/* Extension injects this mapping into WebView: */
:root {
  --bg-base:       var(--vscode-editor-background);
  --bg-surface:    var(--vscode-sideBar-background);
  --bg-elevated:   var(--vscode-input-background);
  --bg-overlay:    var(--vscode-menu-background);
  --bg-subtle:     var(--vscode-list-hoverBackground);

  --text-primary:   var(--vscode-editor-foreground);
  --text-secondary: var(--vscode-descriptionForeground);
  --text-tertiary:  var(--vscode-disabledForeground);
  --text-link:      var(--vscode-textLink-foreground);

  --color-border:   var(--vscode-widget-border, #30363D);

  --accent-primary: var(--vscode-button-background);
  --accent-hover:   var(--vscode-button-hoverBackground);

  --color-success:  var(--vscode-testing-iconPassed);
  --color-warning:  var(--vscode-editorWarning-foreground);
  --color-error:    var(--vscode-editorError-foreground);
}
```

FSM-specific tokens (`--color-history`, `--color-choice`, etc.) are NOT mapped to VS Code
and use fixed values across themes (they are diagram-semantic colours).

## 2.4 Diagram Colour Tokens

| Token | Dark | Light | Usage |
|---|---|---|---|
| `--state-border` | `#30363D` | `#D0D7DE` | Default state box border. |
| `--state-border-active` | `#2F81F7` | `#0969DA` | Active state border (simulator). |
| `--state-border-final` | `#3FB950` | `#1A7F37` | Final state border. |
| `--state-border-error` | `#F85149` | `#CF222E` | State with error breakpoint. |
| `--state-bg` | `#161B22` | `#F6F8FA` | State box background. |
| `--state-bg-active` | `#2F81F712` | `#0969DA12` | Active state overlay. |
| `--state-bg-composite` | `#0D1117` | `#EAEEF2` | Composite state background. |
| `--transition-color` | `#484F58` | `#9198A1` | Default arrow. |
| `--transition-active` | `#2F81F7` | `#0969DA` | Recently executed arrow. |
| `--transition-timed` | `#D29922` | `#9A6700` | Timed transition arrow. |
| `--transition-completion` | `#3FB950` | `#1A7F37` | Completion transition arrow. |
| `--region-divider` | `#21262D` | `#D0D7DE` | Dashed region separator. |
| `--region-label` | `#484F58` | `#9198A1` | Region label text. |

---

# 3. Typography

## 3.1 Font Stacks

```css
--font-ui:   'Geist', 'Inter', 'Segoe UI', system-ui, -apple-system, sans-serif;
--font-mono: 'Geist Mono', 'JetBrains Mono', 'Fira Code', 'Cascadia Code',
             'Consolas', monospace;
```

Web IDE loads Geist and Geist Mono via `@font-face` (self-hosted, no CDN dependency).

VS Code WebViews use:
```css
--font-ui:   var(--vscode-font-family);
--font-mono: var(--vscode-editor-font-family);
```

## 3.2 Type Scale

| Token | Size | Line height | Weight | Usage |
|---|---|---|---|---|
| `--text-xs` | 11px | 1.4 | 400 | Region labels, pseudo-state letters, badges. |
| `--text-sm` | 12px | 1.5 | 400 | Sidebar item labels, trace timestamps. |
| `--text-base` | 13px | 1.5 | 400 | Default UI text, diagnostic messages. |
| `--text-md` | 14px | 1.5 | 400 | State name in diagram, panel section headers. |
| `--text-lg` | 16px | 1.4 | 600 | Panel title, machine name in header. |
| `--text-xl` | 20px | 1.3 | 600 | Modal headings. |

Code and monospace content uses `--font-mono` at the same sizes.

## 3.3 Usage Rules

- State names in diagram: `--text-md`, semibold (600), `--font-ui`.
- Entry/exit annotations in diagram: `--text-xs`, regular (400), italic, `--font-ui`.
- Trace timestamps: `--text-sm`, `--font-mono`, `--text-tertiary`.
- Trace event names: `--text-sm`, `--font-mono`, `--text-primary`.
- Context inspector values: `--text-base`, `--font-mono`.
- Code in editor: `--text-base`, `--font-mono` (Monaco handles this).
- Section headers in panels: `--text-xs`, uppercase, letter-spacing 0.05em,
  `--text-tertiary`, `--font-ui`.

---

# 4. Spacing and Grid

## 4.1 Base Unit

All spacing is based on a **4px grid**. Every padding, margin, gap, and
dimension MUST be a multiple of 4px.

## 4.2 Spacing Scale

| Token | Value | Usage |
|---|---|---|
| `--space-1` | 4px | Inline gaps, icon-to-text spacing. |
| `--space-2` | 8px | Input padding (vertical), small component padding. |
| `--space-3` | 12px | Standard button padding (horizontal). |
| `--space-4` | 16px | Card/panel inner padding. |
| `--space-5` | 20px | Section padding. |
| `--space-6` | 24px | Large section gaps. |
| `--space-8` | 32px | Panel header height. |
| `--space-10` | 40px | Modal padding. |

## 4.3 Component Sizing

| Component | Height |
|---|---|
| Button (default) | 28px |
| Button (small) | 22px |
| Input field | 28px |
| Toolbar | 36px |
| Panel header | 32px |
| Activity bar item | 48px |
| Tab bar | 35px |
| Status bar | 22px |
| Sidebar section header | 24px |
| Dropdown item | 24px |

---

# 5. Component Library

## 5.1 Buttons

### Primary Button

```
Background:  --accent-primary
Text:        --text-inverse  (white)
Border:      none
Height:      28px
Padding:     0 12px
Radius:      6px
Font:        --text-base, 500
Hover:       --accent-hover background
Active:      --accent-strong background
Disabled:    40% opacity, cursor: not-allowed
Focus ring:  2px --accent-primary, 2px offset
```

### Secondary Button

```
Background:  transparent
Text:        --accent-primary
Border:      1px solid --accent-primary
Height:      28px
Padding:     0 12px
Radius:      6px
Hover:       --accent-muted background
Active:      --accent-muted background + --accent-strong border
```

### Ghost Button

```
Background:  transparent
Text:        --text-secondary
Border:      none
Height:      28px
Padding:     0 8px
Radius:      6px
Hover:       --bg-subtle background, --text-primary text
```

### Icon Button

```
Background:  transparent
Width:       28px
Height:      28px
Radius:      6px
Icon size:   16px
Hover:       --bg-subtle background
Active:      --bg-elevated background
```

### Danger Button

```
Background:  --color-error
Text:        white
Hover:       darken 8%
```

## 5.2 Inputs

```
Background:  --bg-elevated
Text:        --text-primary
Border:      1px solid --color-border-strong
Height:      28px
Padding:     0 8px
Radius:      6px
Font:        --text-base
Placeholder: --text-tertiary

Focus:
  border-color: --accent-primary
  box-shadow:   0 0 0 2px --accent-muted

Error:
  border-color: --color-error
  box-shadow:   0 0 0 2px --color-error-muted

Disabled:
  background: --bg-surface
  opacity: 0.5
```

## 5.3 Dropdowns / Select

```
Trigger appearance: identical to Input
Dropdown panel:
  background:  --bg-overlay
  border:      1px solid --color-border
  radius:      6px
  box-shadow:  --shadow-md
  max-height:  240px
  overflow-y:  auto

Item:
  height:     24px
  padding:    0 8px
  font:       --text-base
  color:      --text-primary
  hover:      --bg-subtle background

Active item:
  background: --accent-muted
  color:      --accent-primary
  font-weight: 500
```

## 5.4 Panels and Cards

```
Panel:
  background:  --bg-surface
  border:      1px solid --color-border
  radius:      0 (panels are edge-to-edge)

Panel header:
  background:  --bg-elevated
  height:      32px
  padding:     0 12px
  border-bottom: 1px solid --color-border
  font:        --text-xs, uppercase, letter-spacing 0.05em, --text-tertiary

Card:
  background:  --bg-surface
  border:      1px solid --color-border
  radius:      8px
  padding:     --space-4
```

## 5.5 Shadows

```css
--shadow-sm:  0 1px 2px rgba(0, 0, 0, 0.30);
--shadow-md:  0 4px 8px rgba(0, 0, 0, 0.40);
--shadow-lg:  0 8px 24px rgba(0, 0, 0, 0.50);
```

Light theme: reduce alpha by 40% (0.18, 0.24, 0.30).

## 5.6 Badges

```
Background:  --color-error  (errors), --color-warning (warnings)
Text:        white
Font:        --text-xs, 600
Padding:     0 4px
Height:      16px
Min-width:   16px
Radius:      8px (pill)
Position:    top-right corner of parent, -4px offset each direction
```

## 5.7 Toggles

```
Track width:  32px
Track height: 16px
Track radius: 8px
Track off:    --bg-elevated, border: 1px solid --color-border-strong
Track on:     --accent-primary

Thumb:        white circle, 12px, centered vertically
Thumb off:    translateX(2px)
Thumb on:     translateX(18px)
Transition:   150ms ease
```

## 5.8 Tree Items (Sidebar and Explorer)

```
Height:           22px
Padding:          0 8px 0 (indent * 12px + 8px)
Font:             --text-sm
Color:            --text-secondary
Icon:             16px codicon, --text-tertiary

Hover:
  background:     --bg-subtle
  color:          --text-primary
  icon-color:     --text-secondary

Active (selected):
  background:     --accent-muted
  color:          --text-primary
  icon-color:     --accent-primary
  font-weight:    500

Indent per level: 12px
Chevron size:     12px, rotates 90° on expand (150ms ease)
```

## 5.9 Tabs

```
Tab bar:
  background:  --bg-surface
  border-bottom: 1px solid --color-border
  height:      35px

Tab:
  padding:     0 16px
  font:        --text-sm
  color:       --text-secondary
  border-bottom: 2px solid transparent

Active tab:
  color:        --text-primary
  border-bottom: 2px solid --accent-primary
  background:   --bg-base

Hover:
  background:  --bg-subtle

Close button (✕):
  visible on hover
  codicon: $(close)
  16px, ghost style
```

## 5.10 Toasts

```
Position:      bottom-right, 16px margin
Width:         320px
Border-radius: 8px
Padding:       12px 16px
Box-shadow:    --shadow-lg
Font:          --text-sm
Border-left:   3px solid (semantic colour)

Layout:
  [Icon 16px] [Message] [✕ close button]

Animation:
  Enter: slide-in-from-right, 200ms ease-out
  Exit:  fade-out, 150ms ease-in

Auto-dismiss: 4000ms (paused on hover)
```

---

# 6. Diagram Component Specifications

## 6.1 Simple State Box

```
Min-width:    140px
Border-radius: 6px
Border:       1.5px solid --state-border
Background:   --state-bg
Padding:      8px 12px

Layout (vertical stack):
  ┌─ State Name (--text-md, semibold) ─────────────┐
  │  entry: fn_name   (--text-xs, italic, muted)    │
  │  exit:  fn_name                                 │
  │  ⏱ after 5000ms                                 │
  └─────────────────────────────────────────────────┘

Name colour:  --text-primary
Annotation colour: --text-tertiary
```

## 6.2 Composite State Box

```
Border-radius:  10px
Border:         1.5px solid --state-border
Background:     --state-bg-composite
Header padding: 8px 12px

Layout:
  ┌─ Name (--text-md, semibold) ─── annotations ───┐
  │ ┌─ Region A ─────────┐ ┌─ Region B ───────────┐│
  │ │                     │ │                      ││
  │ └─────────────────────┘ └──────────────────────┘│
  └─────────────────────────────────────────────────┘

Region label:
  --text-xs, uppercase, --region-label, padding 4px 8px
  Border-bottom: 1px dashed --region-divider

Region divider: 1px dashed --region-divider (vertical)
```

## 6.3 Pseudo-State Shapes

### Initial pseudo-state

```
Shape:        Filled circle
Size:         12px diameter
Fill:         --text-primary
No label
```

### Final pseudo-state (final state marker)

```
Shape:        Circle with inner circle
Outer size:   20px diameter
Outer border: 2px solid --color-success
Inner size:   12px diameter
Inner fill:   --color-success
Label:        name below (--text-xs, --text-tertiary)
```

### Shallow history (H)

```
Shape:        Circle
Size:         24px diameter
Border:       2px solid --color-history
Background:   --bg-surface
Label:        "H" centred, --text-sm, 600, --color-history
```

### Deep history (H*)

```
Same as shallow but label "H*"
```

### Choice pseudo-state

```
Shape:        Diamond (rotate square 45°)
Size:         24×24px (before rotation)
Border:       2px solid --color-choice
Background:   --bg-surface
No text label on shape
Name label:   below, --text-xs, --text-tertiary
```

### Junction pseudo-state

```
Shape:        Small filled circle
Size:         10px diameter
Fill:         --text-secondary
No label on shape
```

### Fork / Join

```
Shape:        Filled horizontal bar
Width:        48px
Height:       6px
Fill:         --text-primary
Border-radius: 3px
```

## 6.4 Transition Arrow

```
Stroke:         1.5px
Default colour: --transition-color
Arrowhead:      Filled triangle, 6px wide × 8px tall
Curvature:      Bezier curve; straight for close targets
Self-loop:      Arc on right side of state, 30px radius

Label container:
  Background:   --bg-base, 90% opacity
  Padding:      2px 6px
  Border-radius: 4px
  Font:         --text-xs, --font-mono
  Max-width:    200px (truncate with ellipsis)

Label format:   EVENT [guard] : action
  EVENT:        --text-primary
  [guard]:      --text-tertiary, italic
  : action:     --text-tertiary

Timed transition:
  Stroke:       --transition-timed, dashed (6px dash, 4px gap)
  Label prefix: "⏱ "

Completion transition:
  Stroke:       --transition-completion, dotted
  Label:        "done"
```

## 6.5 Active State Overlay (Simulator)

```
Active state box:
  Border:       2.5px solid --state-border-active
  Background:   --state-bg-active (subtle tint)
  Box-shadow:   0 0 12px --accent-muted

Recently executed transition (600ms animation):
  Stroke animates: dash-offset animation creating "running ants" effect
  Then fades back to normal

Recently exited state (400ms fade):
  Border briefly: --color-warning
  Then fades to default

Breakpoint indicator:
  Red dot: 8px circle, --color-error fill
  Position: top-right corner of state box, -2px offset each direction
```

---

# 7. Iconography

## 7.1 VS Code Extension (Codicons)

All icons in VS Code extension UI MUST use codicons (built-in, theme-aware).

| UI Element | Codicon |
|---|---|
| Activity bar icon | Custom SVG (see §7.3) |
| Simple state | `$(circle-small)` |
| Composite state | `$(symbol-class)` |
| Region | `$(symbol-module)` |
| Event | `$(symbol-event)` |
| Extern function | `$(symbol-method)` |
| Pure extern | `$(symbol-method)` + `$(key)` badge |
| Initial pseudo-state | `$(debug-breakpoint)` |
| Final pseudo-state | `$(debug-breakpoint-log)` |
| History | `$(history)` |
| Machine | `$(circuit-board)` |
| Run | `$(debug-start)` |
| Pause | `$(debug-pause)` |
| Step | `$(debug-step-over)` |
| Stop | `$(debug-stop)` |
| Reset | `$(debug-restart)` |
| Diagram | `$(type-hierarchy)` |
| Simulator | `$(debug-alt)` |
| Refresh | `$(refresh)` |
| Export | `$(export)` |
| Error diagnostic | `$(error)` |
| Warning diagnostic | `$(warning)` |
| Info diagnostic | `$(info)` |
| Hint diagnostic | `$(lightbulb)` |

## 7.2 Web IDE (Custom Icon Set)

Web IDE uses a custom 16×16px SVG icon set with 1.5px strokes, rounded caps.
All icons provided in `--text-primary` colour (CSS `currentColor`).

Required icons:
`state`, `composite-state`, `region`, `event`, `extern`, `machine`,
`history`, `choice`, `fork`, `join`, `initial`, `final`,
`transition`, `timer`, `play`, `pause`, `step`, `stop`, `reset`,
`diagram`, `simulator`, `export`, `settings`, `file`, `folder`,
`chevron-right`, `chevron-down`, `close`, `add`, `refresh`,
`error`, `warning`, `info`, `hint`, `share`, `download`, `upload`.

## 7.3 FSM Studio Logotype

The activity bar and application icon is a custom SVG:

```
Concept: three nodes (states) connected by directed arrows,
         arranged in a compact triangle layout.
         Top node: filled circle (initial).
         Bottom-left: rounded square (composite state).
         Bottom-right: double circle (final state).
         Arrows: two visible arrows between nodes.

Style: 1.5px strokes, rounded caps, accent blue (#2F81F7 / var(--accent-primary)).
       Nodes use --text-primary for fill.
       Background: transparent.
       Size: 16×16px (activity bar), 32×32px (tab), 128×128px (marketplace).
```

---

# 8. Motion and Animation

## 8.1 Principles

- Animation MUST communicate state change, not decorate.
- No animation on text or static content.
- All animations respect `prefers-reduced-motion: reduce` media query (disable all
  non-essential animations when active).

## 8.2 Timing Functions

| Token | Value | Usage |
|---|---|---|
| `--ease-default` | `cubic-bezier(0.16, 1, 0.3, 1)` | Default ease-out. |
| `--ease-in` | `cubic-bezier(0.4, 0, 1, 1)` | Elements exiting. |
| `--ease-linear` | `linear` | Progress bars, dash animations. |

## 8.3 Duration Scale

| Token | Value | Usage |
|---|---|---|
| `--duration-fast` | 100ms | Toggle states, checkbox. |
| `--duration-base` | 150ms | Hover transitions, tab switches. |
| `--duration-slow` | 200ms | Panel expand/collapse, toast entry. |
| `--duration-diagram` | 300ms | Diagram re-render, zoom. |
| `--duration-transition-anim` | 600ms | Executed transition animation in diagram. |

## 8.4 Required Animations

| Event | Animation |
|---|---|
| Active state enters | Border transitions from `--state-border` to `--state-border-active` (150ms). Inner glow fades in (150ms). |
| Active state exits | Reverse of entry (150ms). Brief warning glow (400ms). |
| Transition executes | Dashed dash-offset animation on arrow for 600ms. |
| Toast appears | `translateX(340px) → 0` over 200ms. |
| Toast dismisses | `opacity: 1 → 0` over 150ms. |
| Panel expands | `height: 0 → auto` over 200ms. |
| Chevron rotates | `rotate(0deg) → 90deg` over 150ms. |
| Dropdown opens | `opacity: 0, translateY(-4px) → 1, 0` over 150ms. |

---

# 9. Layout and Composition

## 9.1 Z-Index Scale

| Layer | Z-Index | Elements |
|---|---|---|
| Base | 0 | Normal content. |
| Diagram canvas | 1 | Canvas element. |
| Diagram overlay | 10 | Tooltips on diagram. |
| Dropdown | 100 | Select panels, context menus. |
| Modal backdrop | 200 | Semi-transparent overlay. |
| Modal | 300 | Dialog content. |
| Toast | 400 | Notification stack. |

## 9.2 Scrollbars

Custom scrollbar styling:

```css
::-webkit-scrollbar        { width: 6px; height: 6px; }
::-webkit-scrollbar-track  { background: transparent; }
::-webkit-scrollbar-thumb  {
  background:    var(--color-border);
  border-radius: 3px;
}
::-webkit-scrollbar-thumb:hover {
  background:    var(--color-border-strong);
}
```

## 9.3 Focus Visible

```css
:focus-visible {
  outline: 2px solid var(--accent-primary);
  outline-offset: 2px;
}
```

Never suppress focus outlines. Use `focus-visible` selector to avoid showing
focus rings on mouse clicks.

---

# 10. Responsive Design

The Web IDE adapts its layout to viewport width.
VS Code WebView panels adapt to the panel size automatically (no breakpoints needed).

| Breakpoint | Token | Width |
|---|---|---|
| Compact | `--bp-sm` | < 900px |
| Standard | `--bp-md` | 900–1200px |
| Wide | `--bp-lg` | > 1200px |

See §2.10 of the UI Specification for adaptive layout rules.

---

# 11. Figma Design Kit

A Figma design kit SHOULD be maintained at:

```
FSM Studio / Design System
  └── 01 · Foundations (colours, typography, spacing)
  └── 02 · Components (buttons, inputs, panels, tabs, toasts)
  └── 03 · Diagram Components (states, pseudo-states, arrows)
  └── 04 · Screens (VS Code extension panels, Web IDE screens)
  └── 05 · Icons
```

All tokens in the Figma kit MUST match this document exactly.
The Figma kit is the visual reference; this document is the normative specification.
In case of conflict, this document takes precedence.

---

# 12. Implementation Checklist

Before any UI component ships, verify:

- [ ] All spacing is a multiple of 4px.
- [ ] All colours use design tokens (no hardcoded hex values except in token definitions).
- [ ] All interactive elements have focus-visible styles.
- [ ] All interactive elements are keyboard accessible.
- [ ] Colour alone is not the sole differentiator of any state.
- [ ] All text meets WCAG 2.1 AA contrast requirements (4.5:1 minimum).
- [ ] Animation respects `prefers-reduced-motion`.
- [ ] VS Code WebView uses CSS variable mapping from §2.3.
- [ ] Icon sizes are multiples of 4px.
- [ ] Scrollbars use custom style from §9.2.
