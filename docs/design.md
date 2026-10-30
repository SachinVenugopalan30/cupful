<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Design

People open Cupful many times a day, on a phone, for a few seconds. They check what is left, log what they ate, and put the phone away. The design serves that glance.

## Direction

**Calm, not clinical.** Brand blue is primary: links, focus and primary buttons. Coral, sage and sand-gold sit beside it for categories: meal types, macros and the products area. The light theme sits on a warm sand tint. The dark theme sits on deep blue, not black.

Nothing shouts. Strong colors carry meaning. Emerald means "left". Crimson means "over". Amber-orange means "check this". The owner chose the blue wellness theme, then asked for coral, sage and sand to complement it.

**Numbers first.** The numbers people glance at (kcal left, macro grams, totals) use the system rounded face with tabular digits. On Apple devices that is SF Pro Rounded; elsewhere the system sans. Tabular digits stop totals from jittering as they change. Everything else uses the plain system sans. There are no web fonts, so there is nothing to download and the app works offline.

**The measuring cup.** The one signature element is the `.meter`: a track with graduation marks every 10 percent, like the side of a measuring cup, filling as the day's food is logged. Budget and macro bars use it. Everything around it stays quiet: one card style, hairline borders, a soft blue shadow.

**Sources are labeled.** Every estimated number says where it came from, in words. "Database" in cyan-teal means a matched food or product. "AI estimate" in violet means the model's guess. Color supports the label and is never the only signal.

## Theme

- Light and dark are defined once with CSS `light-dark()`. `color-scheme` picks the side.
- **System** is the default and follows `prefers-color-scheme`. **Light** and **Dark** set `<html data-theme>`.
- The choice lives in Settings → Appearance and is stored per device in `localStorage` under `cupful-theme`, wrapped in try/catch (`src/lib/theme.ts`).
- A small inline script in `app.html` applies the choice before first paint, so there is no flash.
- All text tokens meet WCAG AA (4.5:1) on `--bg`, `--surface` and `--surface-2` in both themes. That includes over, warning, error and the source labels on their soft backgrounds. Meter, chart and input-border colors meet 3:1 against surfaces.

## Palette

| Role | Light / dark mark | Soft | Text |
| --- | --- | --- | --- |
| Brand blue `--accent` | `#2b67a3` / `#84b8ea` | `--accent-soft` | `--accent` |
| Coral `--coral` | `#cb5a3e` / `#ee8a70` | `#fbe9e3` / `#4a2a33` | `--coral-text` `#a8402a` / `#f5a891` |
| Sage `--sage` | `#4f8a6b` / `#7fc09f` | `#e3f0e8` / `#1b3a2c` | `--sage-text` `#2f6a4d` / `#98d3b3` |
| Sand-gold `--gold` | `#9a7710` / `#e2bd5a` | `#f6edd0` / `#3a3220` | `--gold-text` `#6e5410` / `#ecca80` |
| Sand `--sand`, `--sand-soft` | `#efe8da`, `#f8f4ec` / `#22344a`, `#1a2b3d` | page and sunken surfaces (light) | |

Meal types use `--meal-{breakfast,lunch,dinner,snack}` plus `-soft` and `-text`. They are coral, sage, blue and sand-gold, in that order. The Meals group headers use them with a dot and label, and the selected state of the meal-type `.segmented` control uses them.

Macros use `--protein` blue, `--carbs` sage and `--fat` coral, with `--*-text` for labels. Charts use `--chart-1..4`: blue, coral, sage, gold.

Coral marks the products area: `.btn-coral` ("Scan barcode", "Saved products", "Use my product"), the product sheet title (`.sheet-coral`) and the picker panel.

Meaning is separate from decoration. `--over` and `--error` are crimson, deeper and pinker than coral. `--under` is emerald, bluer than sage. `--warning` is amber-orange, redder than gold. Meaning is always also in words.

Rule: blue stays primary. Aim for two or three accents per screen. Accent colors are for categories, never for state.

## Charts

LayerChart draws Trends. It is pinned at `2.8.0` and uses the SVG layer. `ChartCard` maps its CSS variables onto the tokens, so both themes follow `light-dark()` with no chart-specific theme code.

- Intake bars use `--chart-intake` (blue). Bars over budget use `--chart-over` (crimson). The per-day budget tick uses `--chart-budget` (navy or near-white) with a halo in the card color, so it reads over any bar.
- Weight points use `--chart-weight` (violet) and the 7-day average line uses `--chart-average` (sage). Steps use `--chart-steps` (emerald). Grid lines use `--chart-grid` and axis text uses `--chart-axis`.
- Color is never alone. A legend names each mark with a shape and a word. Over-budget days also say "over" in the readout.
- LayerChart's floating tooltip is off because it moves under the finger. The day touched, hovered or clicked is written as a sentence in a fixed two-line readout above the chart, for example "Fri 16 Oct: 1797 kcal eaten of 2154 kcal, 357 left." The sentence stays after the finger lifts, so tapping never shifts the layout.
- The plot is `role="img"` with an `aria-label` summary of range, counts and averages. Keyboard users get the summary only. There is no keyboard day-stepping yet.
- Each chart has its own `.empty` sentence that says what to log to fill it. The average card does the same. Gaps, such as no meals or no weigh-in, are gaps, never zeros.
- The range control (7d, 30d, 90d, 365d) is a `.segmented` control remembered per device. Bars thin to hairlines past 120 days.
- The Charts chunk loads when Trends opens, so Today does not carry the library.

## Tokens

Tokens live in `web/src/lib/styles/tokens.css`.

| Group | Tokens |
| --- | --- |
| Surfaces, text | `--bg`, `--surface`, `--surface-2` (sunken), `--border` (decorative), `--border-strong` (inputs, 3:1), `--text`, `--text-muted` |
| Brand | `--accent`, `--accent-hover`, `--on-accent`, `--accent-soft`, `--focus-ring` |
| Meaning | `--under`/`--under-soft` (left, success), `--over`/`--over-soft`, `--warning`/`--warning-soft`, `--error`/`--error-soft` |
| Palette | `--coral`, `--sage`, `--gold` plus `-soft` and `-text`; `--sand`, `--sand-soft`, `--meal-*`, `--protein-text`/`--carbs-text`/`--fat-text`, `--chart-1..4` |
| Sources | `--src-db`/`--src-db-soft` ("Database"), `--src-ai`/`--src-ai-soft` ("AI estimate") |
| Macros, charts | `--protein`, `--carbs`, `--fat`, `--chart-intake`, `--chart-budget`, `--chart-weight`, `--chart-average`, `--chart-steps`, `--chart-over`, `--chart-grid`, `--chart-axis` |
| Type | `--font-sans`, `--font-numeric`; `--text-xs` 13, `--text-sm` 14, `--text-md` 16, `--text-lg` 20, `--text-xl` 25, `--text-2xl` 32, `--text-hero` 52 px; `--leading-tight`, `--leading`; `--weight-regular`, `--weight-medium`, `--weight-bold` |
| Space | `--space-1` through `--space-8` (4, 8, 12, 16, 24, 32, 40, 48 px), `--gutter`, `--content-max` (40rem), `--touch` (44 px) |
| Shape | `--radius-sm` (chips), `--radius-md` (buttons, inputs), `--radius-lg` (cards), `--radius-sheet` (dialogs), `--radius-pill`, `--shadow-card`, `--shadow-sheet` |
| Motion | `--dur-fast` 120, `--dur-med` 220, `--dur-slow` 420 ms, `--ease`. All are 0 under `prefers-reduced-motion` |

## Global classes

Global classes live in `web/src/lib/styles/global.css`. Screens use them and add scoped styles only for layout. There is no CSS framework, UI kit or icon pack. Icons are a few inline SVG paths using `currentColor`.

- Layout: `.stack`, `.row`.
- Text: `.num` (rounded tabular numerals), `.muted`, `.small`, `.visually-hidden`.
- Cards: `.card`.
- Buttons: `.btn` with `.btn-primary`, `.btn-quiet` or `.btn-danger`, and `.btn-block`.
- Forms: `.field` (label, control, `.hint`). Native inputs are styled globally. Checkboxes and radios are drawn from tokens, not `accent-color`, so they match both themes. Invalid fields use `[aria-invalid='true']`.
- `.segmented` turns a radio group into a segmented control. The selected option gets `--accent-soft`, an `--accent` outline and bold text.
- Lists and chips: `.list` (rows with dividers), `.chip` with `.chip-db`, `.chip-ai`, `.chip-warning`, `.chip-error` or `.chip-over`.
- Meters: `.meter` with `style="--value: 0.62; --fill: var(--protein)"`. Add `.is-over` past 1.
- States: `.notice` with `.notice-error`, `.notice-warning` or `.notice-info`; `.empty`; `.skeleton`.
- Sheets: `dialog.sheet` is a native `<dialog>` shown as a bottom sheet on phones and centered on wide screens. It scrolls itself. Its title row is a `<header class="sheet-head">` that stays in view while the sheet scrolls.

## Component rules

- Touch targets are at least 44 by 44 px (`--touch`). Inputs use 16 px text so iOS does not zoom.
- The app works at 360 px wide with no sideways scroll; the e2e test checks this. Phones get a bottom tab bar. From 48rem there is a top bar. From 60rem Today splits into budget beside logging and meals.
- Focus is always visible: a 3 px `--focus-ring` outline via `:focus-visible`. Never remove it without a replacement.
- Every control has a label: a visible `<label>`, or `.visually-hidden` when the context is obvious, such as the date picker. Icon-only buttons get `aria-label`.
- Color is never the only signal. "Over" says "over", sources say their name, flags are words.
- Errors use a `.notice-error` with `role="alert"`, next to what failed. The message says what happened and what to do next, in plain words (`errorMessage()` in `src/lib/api.ts`). Errors do not apologize. When the fix is in Settings, the message links there.
- Empty states use `.empty` with one sentence inviting the next action, such as "Nothing logged yet. Describe your breakfast above.", and optionally one button.
- Loading uses `.skeleton` blocks the size of what is coming, with `aria-busy="true"` on the container. No spinners for waits under a second.
- Motion answers an action, such as a sheet opening or a meter filling after a save, and uses the duration tokens. Reduced motion turns it off. There are no entrance animations.
- Disclosures and pickers use a quiet `.btn` with `aria-expanded` to reveal extra inputs ("Edit numbers") or an inline panel on `--surface-2` ("Use my product": a search field and a `.list` of buttons) inside the item card. There are no nested dialogs.
- Copy uses sentence case and plain verbs. Buttons say what happens ("Save meal", "Scan barcode"), and an action keeps its name through the flow.
