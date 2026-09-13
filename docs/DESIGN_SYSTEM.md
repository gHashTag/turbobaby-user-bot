# Woody Weed Bot — Design System

> Dioxus 0.6 + WASM · Telegram WebApp · Dark Mode First

---

## Table of Contents

1. [Color Tokens](#1-color-tokens)
2. [Neon Glow Tokens](#2-neon-glow-tokens)
3. [Spacing Scale](#3-spacing-scale)
4. [Typography](#4-typography)
5. [Border Radius](#5-border-radius)
6. [Gradients](#6-gradients)
7. [Rarity System](#7-rarity-system)
8. [Component Library](#8-component-library)
   - [Button](#button)
   - [Card](#card)
   - [Badge](#badge)
   - [Toast](#toast)
   - [EmptyState](#emptystate)
   - [Skeleton](#skeleton)
   - [ProgressBar](#progressbar)
   - [Chip / ChipGroup](#chip--chipgroup)
   - [RarityGlow](#rarityglow)
9. [CSS Animations](#9-css-animations)
10. [File Structure](#10-file-structure)

---

## 1. Color Tokens

All tokens are defined in `styles/variables.css` under `:root`.

### Brand Colors

| Token | Value | Description |
|---|---|---|
| `--color-primary` | `#22c55e` | Weed green — primary CTA |
| `--color-primary-light` | `#4ade80` | Lighter green hover |
| `--color-primary-dark` | `#16a34a` | Darker green active |
| `--color-accent-purple` | `#a855f7` | Purple accent |
| `--color-accent-gold` | `#fbbf24` | Gold accent |
| `--color-cyan` | `#00e5ff` | Info / quest accent |
| `--color-red` | `#ff4757` | Danger / error |

### Background Colors

| Token | Value | Description |
|---|---|---|
| `--color-bg-dark` | `#0a0a0a` | Deepest dark |
| `--color-bg` | `#0f0f1a` | App background |
| `--color-bg2` | `#1a1a2e` | Elevated surface |
| `--color-card` | `#16213e` | Card background |
| `--color-bg-card` | `rgba(255,255,255,0.05)` | Glass card base |

### Text Colors

| Token | Value |
|---|---|
| `--color-text` | `#e8e8e8` |
| `--color-text2` | `#888888` |
| `--color-text-muted` | `#555577` |
| `--color-text-inverse` | `#0a0a0a` |

---

## 2. Neon Glow Tokens

```css
--neon-green-glow:    0 0 12px rgba(34, 197, 94, 0.6)
--neon-green-glow-lg: 0 0 24px rgba(34, 197, 94, 0.7), 0 0 48px rgba(34, 197, 94, 0.3)
--neon-green-glow-sm: 0 0 6px rgba(34, 197, 94, 0.5)

--neon-purple-glow:   0 0 12px rgba(168, 85, 247, 0.6)
--neon-gold-glow:     0 0 12px rgba(251, 191, 36, 0.6)
--neon-red-glow:      0 0 12px rgba(255, 71, 87, 0.6)
--neon-cyan-glow:     0 0 12px rgba(0, 229, 255, 0.6)
```

---

## 3. Spacing Scale

4 px base scale. Both `--space-N` and legacy names available.

| Token | Value | Legacy |
|---|---|---|
| `--space-1` | `4px` | `--space-xs` |
| `--space-2` | `8px` | `--space-sm` |
| `--space-3` | `12px` | — |
| `--space-4` | `16px` | `--space-md` |
| `--space-5` | `20px` | — |
| `--space-6` | `24px` | `--space-lg` |
| `--space-7` | `28px` | — |
| `--space-8` | `32px` | `--space-xl` |

---

## 4. Typography

| Token | Value |
|---|---|
| `--font-display` | `'Press Start 2P', monospace` |
| `--font-body` | `'Inter', system-ui, sans-serif` |
| `--font-mono` | `'Courier New', monospace` |
| `--font-size-xs` | `10px` |
| `--font-size-sm` | `13px` |
| `--font-size-base` | `15px` |
| `--font-size-lg` | `18px` |
| `--font-size-xl` | `24px` |
| `--font-size-2xl` | `32px` |

---

## 5. Border Radius

| Token | Value |
|---|---|
| `--radius-xs` | `2px` |
| `--radius-sm` | `4px` |
| `--radius-md` | `8px` |
| `--radius-lg` | `12px` |
| `--radius-xl` | `16px` |
| `--radius-2xl` | `24px` |
| `--radius-full` | `9999px` |

---

## 6. Gradients

| Token | Direction |
|---|---|
| `--gradient-green` | Primary → dark green |
| `--gradient-purple` | Light purple → dark purple |
| `--gradient-gold` | Light gold → dark gold |
| `--gradient-neon` | Green → purple → gold |
| `--gradient-dark` | Dark → bg2 |
| `--gradient-card` | Subtle glass gradient |

---

## 7. Rarity System

Four tiers used for NFT-style items, plants, and achievements.

| Tier | Color | CSS Class | Glow |
|---|---|---|---|
| Common | `#6b7280` (grey) | `.rarity-glow-common` | Subtle grey |
| Rare | `#3b82f6` (blue) | `.rarity-glow-rare` | Blue pulse |
| Epic | `#a855f7` (purple) | `.rarity-glow-epic` | Purple pulse |
| Legendary | `#fbbf24` (gold) | `.rarity-glow-legendary` | Gold breathing animation |

---

## 8. Component Library

All components live in `src/ui/components/`.

---

### Button

**File:** `src/ui/components/button.rs`

```rust
use crate::ui::components::{Button, ButtonVariant, ButtonSize};

// Primary
rsx! {
    Button { variant: ButtonVariant::Primary, label: "Buy Now".to_string(), {} }
}

// Neon Green
rsx! {
    Button { variant: ButtonVariant::NeonGreen, label: "Add to Cart".to_string(), {} }
}

// Neon Purple
rsx! {
    Button { variant: ButtonVariant::NeonPurple, label: "Unlock".to_string(), {} }
}

// Neon Gold — large
rsx! {
    Button {
        variant: ButtonVariant::NeonGold,
        size: ButtonSize::Large,
        label: "Checkout".to_string(),
        {}
    }
}
```

**Variants:** `Primary`, `Secondary`, `Outline`, `Ghost`, `Danger`, `Pixel`, `NeonGreen`, `NeonPurple`, `NeonGold`, `NeonRed`

**Sizes:** `Small`, `Medium` (default), `Large`, `Full`

---

### Card

**File:** `src/ui/components/card.rs`

```rust
use crate::ui::components::{Card, CardVariant};

// Default
rsx! {
    Card { "Content here" }
}

// Glassmorphism
rsx! {
    Card { variant: CardVariant::Glass, "Glass card" }
}

// Glass with green tint
rsx! {
    Card { variant: CardVariant::GlassGreen, clickable: true, "Green glass" }
}
```

**Variants:** `Default`, `Product`, `Plant`, `Quest`, `Member`, `Glass`, `GlassGreen`, `GlassPurple`, `GlassGold`

---

### Badge

**File:** `src/ui/components/badge.rs`

```rust
use crate::ui::components::{Badge, BadgeVariant, BadgeSize};

rsx! {
    Badge { variant: BadgeVariant::Legendary, label: "Legendary".to_string() }
    Badge { variant: BadgeVariant::Epic, label: "Epic".to_string() }
    Badge { variant: BadgeVariant::Rare, label: "Rare".to_string() }
    Badge { variant: BadgeVariant::Common, label: "Common".to_string() }
    Badge { variant: BadgeVariant::Success, label: "Done".to_string() }
    Badge { variant: BadgeVariant::Warning, label: "Caution".to_string() }
    Badge { variant: BadgeVariant::Info, label: "Info".to_string() }
    Badge { variant: BadgeVariant::Error, label: "Failed".to_string(), size: BadgeSize::Large }
}
```

**Variants:** `Neutral`, `Common`, `Rare`, `Epic`, `Legendary`, `Success`, `Warning`, `Info`, `Error`

---

### Toast

**File:** `src/ui/components/toast.rs`

```rust
use crate::ui::components::{Toast, ToastKind, ToastContainer};

rsx! {
    ToastContainer {
        Toast {
            kind: ToastKind::Success,
            message: "Заказ оформлен!".to_string(),
            on_close: move |_| { /* dismiss */ }
        }
        Toast {
            kind: ToastKind::Error,
            message: "Что-то пошло не так".to_string(),
            on_close: move |_| {}
        }
    }
}
```

**Kinds:** `Info`, `Success`, `Warning`, `Error`

---

### EmptyState

**File:** `src/ui/components/empty_state.rs`

```rust
use crate::ui::components::{EmptyState, Button, ButtonVariant};

rsx! {
    EmptyState {
        icon: "🛒".to_string(),
        title: "Корзина пуста".to_string(),
        description: "Добавьте товары из каталога".to_string(),
        action: rsx! {
            Button { variant: ButtonVariant::NeonGreen, label: "В каталог".to_string(), {} }
        }
    }
}
```

**Props:** `icon: String`, `title: String`, `description: String`, `action: Option<Element>`, `glass: bool`

---

### Skeleton

**File:** `src/ui/components/skeleton.rs`

```rust
use crate::ui::components::{Skeleton, SkeletonShape, SkeletonRow};

// Card placeholder
rsx! {
    Skeleton { shape: SkeletonShape::Card }
}

// Text row with custom dimensions
rsx! {
    Skeleton { width: "200px".to_string(), height: "16px".to_string() }
}

// Multiple text lines
rsx! {
    SkeletonRow { lines: 4 }
}
```

**Shapes:** `Rectangle`, `Text`, `TextSm`, `Title`, `Avatar`, `AvatarLg`, `Card`, `Button`

---

### ProgressBar

**File:** `src/ui/components/progress_bar.rs`

```rust
use crate::ui::components::{ProgressBar, ProgressColor, ProgressSize};

// Basic
rsx! {
    ProgressBar { value: 65.0, max: 100.0 }
}

// Gold large with label
rsx! {
    ProgressBar {
        value: 420.0,
        max: 1000.0,
        color: ProgressColor::Gold,
        size: ProgressSize::Large,
        label: "До Золотого".to_string(),
        show_percent: true
    }
}
```

**Colors:** `Green`, `Purple`, `Gold`, `Neon`  
**Sizes:** `Small`, `Default`, `Large`, `XLarge`

---

### Chip / ChipGroup

**File:** `src/ui/components/chip.rs`

```rust
use crate::ui::components::{Chip, ChipGroup, ChipColor};

let mut selected = use_signal(|| "sativa".to_string());

rsx! {
    ChipGroup {
        Chip {
            label: "Sativa".to_string(),
            selected: *selected.read() == "sativa",
            icon: "☀️".to_string(),
            on_click: move |_| selected.set("sativa".to_string())
        }
        Chip {
            label: "Indica".to_string(),
            selected: *selected.read() == "indica",
            color: ChipColor::Purple,
            on_click: move |_| selected.set("indica".to_string())
        }
    }
}
```

**Colors:** `Green`, `Purple`, `Gold`

---

### RarityGlow

**File:** `src/ui/components/rarity_glow.rs`

```rust
use crate::ui::components::{RarityGlow, Rarity};

rsx! {
    RarityGlow { rarity: Rarity::Legendary,
        Card { variant: CardVariant::Default,
            "OG Kush — Legendary"
        }
    }
}

// From dynamic string
let rarity = Rarity::from_str(&strain.rarity);
rsx! {
    RarityGlow { rarity, { content } }
}
```

**Rarities:** `Common`, `Rare`, `Epic`, `Legendary`

---

## 9. CSS Animations

Defined in `styles/components.css`:

| Keyframe | Usage |
|---|---|
| `pulse-glow` | Neon green pulsing glow |
| `pulse-glow-purple` | Purple variant |
| `pulse-glow-gold` | Gold variant |
| `fade-in` | Opacity 0 → 1 |
| `slide-up` | Translate Y + fade |
| `slide-down` | Translate Y (from top) + fade |
| `shimmer` | Skeleton loading sweep |
| `spin` | 360° rotation (spinner) |
| `pop-in` | Scale 0.75 → 1 |
| `bounce-in` | Spring scale in |
| `toast-slide-in` | Slide from right |
| `toast-slide-out` | Slide out to right |
| `rarity-pulse-legendary` | Gold border breathing |

**Utility classes:**

```html
<div class="animate-fade-in">...</div>
<div class="animate-slide-up">...</div>
<div class="animate-pop-in">...</div>
<div class="animate-pulse-green">...</div>
<div class="text-glow-green">Glowing text</div>
```

---

## 10. File Structure

```
turbobaby-bot/
├── styles/
│   ├── variables.css      ← Design tokens (expanded)
│   ├── main.css           ← Base styles + legacy components
│   ├── components.css     ← New component styles (NEW)
│   └── simple.css         ← Legacy minimal
├── src/ui/components/
│   ├── mod.rs             ← Re-exports all components
│   ├── button.rs          ← Button + NeonGreen/Purple/Gold/Red variants
│   ├── card.rs            ← Card + Glass variants
│   ├── badge.rs           ← Badge (rarity + status) (NEW)
│   ├── toast.rs           ← Toast notifications (NEW)
│   ├── empty_state.rs     ← Empty state with CTA (NEW)
│   ├── skeleton.rs        ← Skeleton shimmer loader (NEW)
│   ├── progress_bar.rs    ← Progress bar with gradient (NEW)
│   ├── chip.rs            ← Chip / filter tags (NEW)
│   ├── rarity_glow.rs     ← Rarity glow wrapper (NEW)
│   ├── modal.rs
│   ├── nav.rs
│   ├── input.rs
│   ├── loading.rs
│   ├── strain_card.rs
│   ├── cart_item.rs
│   ├── strain_grid.rs
│   └── plant_cell.rs
├── docs/
│   └── DESIGN_SYSTEM.md   ← This file
├── storybook.html         ← Visual component preview
└── index.html             ← App entry (loads all CSS)
```
