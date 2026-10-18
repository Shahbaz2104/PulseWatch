---
name: PulseWatch
description: The Midnight Control Room — cyberpunk-infused observability for self-hosted uptime monitoring.
colors:
  signal-emerald: "#10b981"
  alarm-rose: "#fb7185"
  watch-amber: "#fbbf24"
  unknown-slate: "#94a3b8"
  midnight-ground: "#0b0f17"
  midnight-raised: "#0e1420"
  fog-text: "#e2e8f0"
  cool-paper: "#f5f7fb"
  ink: "#0d1526"
  hairline-dark: "rgb(255 255 255 / 0.07)"
  hairline-light: "#dce3ee"
  accent-sky: "#38bdf8"
typography:
  display:
    fontFamily: "Geist Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "1.25rem"
    fontWeight: 600
    letterSpacing: "-0.01em"
  title:
    fontFamily: "Geist Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 500
  body:
    fontFamily: "Geist Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: "Geist Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "0.6875rem"
    fontWeight: 500
    letterSpacing: "0.14em"
  chip:
    fontFamily: "Geist Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 500
  data:
    fontFamily: "Geist Mono Variable, ui-monospace, SF Mono, monospace"
    fontSize: "0.875rem"
    fontWeight: 500
    fontFeature: "tabular-nums"
  metric:
    fontFamily: "Geist Mono Variable, ui-monospace, SF Mono, monospace"
    fontSize: "1.5rem"
    fontWeight: 600
    fontFeature: "tabular-nums"
rounded:
  sm: "8px"
  md: "10px"
  lg: "12px"
  xl: "16px"
  pill: "9999px"
spacing:
  sm: "8px"
  md: "16px"
  lg: "24px"
  xl: "32px"
components:
  button-primary:
    backgroundColor: "{colors.signal-emerald}"
    textColor: "#022c22"
    rounded: "{rounded.md}"
    typography: "{typography.body}"
    padding: "8px 16px"
  button-outline:
    backgroundColor: "transparent"
    textColor: "{colors.fog-text}"
    rounded: "{rounded.md}"
    typography: "{typography.body}"
    padding: "8px 12px"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.unknown-slate}"
    rounded: "{rounded.md}"
    size: "36px"
  input:
    backgroundColor: "rgb(15 23 42 / 0.58)"
    textColor: "{colors.fog-text}"
    rounded: "{rounded.md}"
    typography: "{typography.body}"
    padding: "8px 12px"
  card:
    backgroundColor: "rgb(15 23 42 / 0.58)"
    textColor: "{colors.fog-text}"
    rounded: "{rounded.lg}"
    padding: "16px"
  nav-item:
    backgroundColor: "transparent"
    textColor: "{colors.unknown-slate}"
    rounded: "{rounded.md}"
    typography: "{typography.body}"
    padding: "8px 10px"
  status-pill:
    backgroundColor: "rgb(30 41 59 / 0.55)"
    textColor: "{colors.signal-emerald}"
    rounded: "{rounded.pill}"
    typography: "{typography.chip}"
    padding: "2px 8px"
---

# Design System: PulseWatch

## Overview

**Creative North Star: "The Midnight Control Room"**

PulseWatch looks like an operator's console kept at 3am: a layered midnight ground with slow ambient light, glass chrome that holds still, and instruments that tell the truth. The intensity contract is "data calm, chrome loud" — the shell (rails, backdrop wash, boot sequence, empty states) is allowed atmosphere and glow, while tables, charts, and metrics stay quiet, dense, and monospace-precise. Saturation is earned only by state: emerald means up, rose means down, amber means degraded; everything else is slate.

The system rejects both marketing gloss and raw utilitarian gray. It is cyberpunk-infused through material (layered dark gradients, hairline glass, a slow aurora wash, a wireframe-orb boot) rather than through neon overload — the brief's pinned direction, applied with restraint. Light theme is a first-class remap of the same roles on cool paper, not an afterthought. Depth is declared once: cards declare elevation with a hairline border, floating layers declare it with a soft offset shadow, and nothing doubles up.

**Key Characteristics:**
- Hairline-first elevation: borders carry structure; `shadow-pop` is reserved for floating layers only.
- Status palette as the only loud color; chrome stays monochrome slate/emerald accents.
- Geist + Geist Mono with tabular numerals everywhere numbers live.
- Instrument density: 11px uppercase micro-labels, 12px radii, 16–24px rhythm.
- One signature ambient layer (canvas aurora) + one signature data component (the padded uptime strip).

## Colors

The palette is status-semantics-first: three signal hues on a midnight/cool-paper neutral field, with charts borrowing the same hues so a graph never introduces a voice the pills don't already speak.

### Primary
- **Signal Emerald** (#10b981): primary accent and "up" status — buttons, active nav indicator, focus rings, logo pulse, response-time chart line, ring-pulse. Dark-theme `--primary`; light theme deepens to #059669 for contrast on paper.
- **Accent Sky** (#38bdf8): second chart series and the cool radial tint in the dark body gradient. Never used for status.

### Status (semantic, reserved)
- **Alarm Rose** (#fb7185): "down" status — down pills, failing strip segments, incident state. Light-theme destructive #e11d48 sits in the same role.
- **Watch Amber** (#fbbf24): "degraded" status — warning chips, partial strip segments.
- **Unknown Slate** (#94a3b8): unknown/paused status and secondary icon chrome.

### Neutral
- **Midnight Ground** (#0b0f17 → #0e1420): dark body, a fixed vertical gradient with two radial tints (emerald 9%, sky 6%) at the top corners.
- **Fog Text** (#e2e8f0): dark-theme foreground and primary text.
- **Cool Paper** (#f5f7fb → #eef2f8): light body gradient; **Ink** (#0d1526) is its text.
- **Hairline** (dark `rgb(255 255 255 / 0.07)`, light #dce3ee): every card and divider edge.

### Named Rules
**The Status-Voice Rule.** Saturated color appears only where it carries check state (up/degraded/down/unknown) or the single primary action. If a screen has three colors, two of them are probably lying.

**The Chrome-Atmosphere Rule.** Gradients, aurora, and blur belong to the shell layer (body, sidebar, header, overlays, boot) — never behind a data table or chart. Data areas sit on flat card surfaces.

## Typography

**Display Font:** Geist Variable (ui-sans-serif, system-ui fallback)
**Body Font:** Geist Variable (same stack)
**Data/Mono Font:** Geist Mono Variable (ui-monospace, SF Mono fallback)

**Character:** Geist's grotesque neutrality reads as instrumentation, not brand voice; Geist Mono with tabular figures makes every latency and count line up like a readout. Self-hosted (no network fonts).

### Hierarchy
- **Display** (600, 20px / `text-xl`, tracking −0.01em): page titles only (`h1` in PageHeader).
- **Title** (500, 14px): card titles ("Uptime — last 24h"), section labels.
- **Body** (400, 14px/1.5): descriptions, table cells, list copy; muted foreground #94a3b8 (dark) / #5b6b85 (light) for secondary lines.
- **Label** (500, 11px, 0.14em tracking, uppercase — `.label-cap`): metric labels, column caps, version tag. Never longer than 3–4 words.
- **Data** (500, 14px mono, tabular): URLs, timestamps, table numbers.
- **Metric** (600, 24px mono, tabular): KPI values; NumberFlow animates the digits in place without reflow.

### Named Rules
**The Tabular Rule.** Any number that can change in place — metrics, table columns, durations — renders with `font-variant-numeric: tabular-nums`. Jittering digits read as unreliable data.

## Layout

Desktop is a two-region shell: a **w-56 (224px) sticky glass sidebar** that collapses to **w-14 (56px)**, and a content column (`max-w-6xl`, px-8, py-6) topped by a **56px sticky glass header** carrying breadcrumbs, ⌘K search, and theme toggle. Sidebar and header both use `backdrop-blur-xl` over translucent token backgrounds — the one permitted blur.

Density rhythm: 4px base scale; card padding 16px (`py-4 px-4`), grid gaps 16px (`gap-4`), section gaps 24px (`space-y-6`). KPI rows are `sm:grid-cols-4` (counts) over `sm:grid-cols-2` (rate metrics); report rows are `lg:grid-cols-2`.

Responsive: sidebar hides below `md` (768px) and becomes a hamburger `Sheet` with the same nav; tables scroll horizontally inside `overflow-x-auto`; KPI grids collapse to single column; the header keeps only menu, search icon, and theme toggle at narrow widths. Large data tables (Reports) paginate client-side at 20 rows with a `Page x of y · n monitors` footer.

## Elevation & Depth

The system is **hairline-first with a single floating shadow**: cards and panels declare elevation with a 1px border and a slightly translucent surface — no resting shadows anywhere. Depth beyond that comes from layering: fixed gradient body → ambient canvas aurora (z −1) → content → glass shell → `shadow-pop` floating layers (command palette, popovers, dialogs, tooltips).

### Shadow Vocabulary
- **shadow-pop, dark** (`0 20px 50px -12px rgb(0 0 0 / 0.55)`): command palette, dropdowns, tooltips, popovers — anything that detaches from the page.
- **shadow-pop, light** (`0 16px 40px -12px rgb(15 23 42 / 0.18)`): same surfaces on paper.
- **Glass blur** (`blur(16px) saturate(140%)`): shell surfaces only (sidebar, header, mobile sheet).

### Named Rules
**The Declare-Elevation-Once Rule.** Cards get a border and never a shadow; floating layers get `shadow-pop` and never a border-as-lift. Mixing both on one surface is banned — it is how dashboards start looking like sticker books.

## Shapes

Gently rounded instrument panels: a **12px base radius** (`--radius: 0.75rem`) with derived steps — 8px `sm`, 10px `md` (buttons, inputs, nav rows), 12px `lg` (cards), 16px `xl` (large overlays), full pill for status chips and dots. Borders are always 1px hairlines; there are no heavy outlines or double borders. Uptime-strip segments are 4px-radius bars with a 2px gap. Focus is a 2px emerald outline, 2px offset (buttons additionally use a 3px ring at 50% alpha).

## Components

### Buttons
- **Shape:** 10px radius; default `h-9 px-4`, sm `h-8 px-3`, xs `h-6 px-2`.
- **Primary:** Signal Emerald fill with near-black-green text (#022c22) — reserved for "Add monitor" / "Save" / commit actions. Hover brightens slightly; disabled at 50% opacity.
- **Outline:** transparent with hairline border and Fog text; hover fills with sidebar accent. Secondary actions and pagination.
- **Ghost:** no border, muted icon at 16px; header icon buttons (⌘K, theme, collapse).
- **Focus:** `ring-[3px] ring-ring/50` plus border shift — keyboard-visible only.

### Inputs / Fields
10px radius, translucent dark surface (or white/92% on paper), 1px input border, 8×12px padding. Focus: emerald border + 3px ring at 50%. Errors: destructive border + ring at 20%, message in rose below.

### Cards / Containers
12px radius, hairline border, `--card` surface (slate-900 at 58% on dark — flat, **no blur**). Titles are 14px medium; `.card-hover` brightens the border toward emerald (32% mix) over 160ms — never lifts. Metric cards: 11px uppercase label, 24px mono value, muted hint line, bordered 36px icon chip that tints by status tone.

### Navigation
Sidebar rows: 10px radius, 8×10px padding, slate-400/70 text; hover fills accent; active state adds accent background **plus a 2px emerald bar on the left edge** (spring-animated `layoutId`), so position is never color-only. The logo is a bordered emerald-tinted square with a Heartbeat icon and a pulsing emerald dot (`ring-pulse`, 1.8s).

### Status Pills
Full pill, 1px border, 2×8px padding, 12px medium text (`text-xs`), 6px dot. Tints: emerald (Up), rose (Down), amber (Degraded), slate (Paused/Unknown). The down/up dot may carry `animate-ring-pulse` — a soft expanding ring in `currentColor`, no neon glow. State pills in tables read **Open** (rose) / **Resolved** (emerald), never raw enum strings.

### Signature: Uptime Strip
The 24-slot hourly strip is the product's visual signature: fixed-width segments with 2px gaps, emerald/amber/rose for up/degraded/down, and **muted slots for "no checks"** — padded to a full 24h window so silence never masquerades as uptime or failure. Hover dims sibling segments and shows a floating tooltip (shadow-pop) with bucket time, uptime %, and check counts. X-axis labels are local HH:mm at both ends.

### Shell Chrome (ambient + boot)
The `ambient-layer` canvas paints three slow-orbit radial washes (emerald, indigo, teal — 17% alpha dark / 9% light) at half resolution, ~30fps, static under `prefers-reduced-motion`. First visit per session plays a ≤1.7s boot curtain: three.js emerald wireframe orb, anime.js letter-stagger wordmark, GSAP exit that hands off to the shell (click to skip; disabled for tests and reduced-motion).

## Do's and Don'ts

### Do:
- **Do** keep resting surfaces shadow-free — hairline border + translucent fill declare elevation.
- **Do** reserve saturated emerald/rose/amber for check state and the single primary action.
- **Do** render every mutable number with tabular numerals (`.tabular`, mono, `font-variant-numeric`).
- **Do** pad uptime/history series to their full window with explicit "no checks" slots.
- **Do** show long report tables paginated at 20 rows with a count footer.
- **Do** keep `.label-cap` at 11px/0.14em uppercase, 3–4 words maximum.
- **Do** use `shadow-pop` on floating layers (palette, popovers, tooltips, dialogs).

### Don't:
- **Don't** put a drop shadow on a card, or a hairline-lift on a floating layer — pick one elevation language per surface.
- **Don't** use blur/glass behind data tables or charts; atmosphere belongs to the shell.
- **Don't** introduce chart colors outside the token set (emerald, sky, amber, violet, rose are the whole alphabet).
- **Don't** animate digits in a way that reflows their box, or drop `tabular-nums` from live numbers.
- **Don't** use status hues as decorative accents, hover colors, or section backgrounds.
- **Don't** show raw enum values (states, UUIDs) where labeled words belong — breadcrumb "Monitor", pill "Open".
