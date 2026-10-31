<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Design

People open Cupful many times a day, on a phone, for a few seconds. They check what is left, log what they ate, and put the phone away. The design serves that glance.

## Direction

**Mineral.** Cool pale stone, white panels, one deep sea blue. The light theme sits on `#f2f5f6`, not a warm tint. The dark theme sits on `#0e1418`, a green-black rather than navy. The palette is quiet and slightly cool, like a kitchen scale and a glass of water.

**Pantry layout.** Soft surfaces, generous spacing and large radii. White cards sit on the pale ground with a hairline border and no shadow. Desktop gets a top bar with the wordmark, four plain links and a user chip. Phones get bottom tabs with an icon and a short label. The current route is neutral: `--surface-2` with normal text, never an accent fill.

Nothing shouts. Strong colors carry meaning. Teal means "left". Brick means "over". Amber means "check this". Data hues come from pantry food: blue protein, ochre carbs, brick fat, teal steps, berry weight, and orange, teal, blue and gold for the four meals.

**Numbers first.** Every number uses Manrope with tabular figures, so totals do not jitter as they change. The numbers people glance at are large: 28 px for a page title and 44 px for kcal left.

**The measuring cup.** The one signature element is the `.meter`: a track with graduation marks every 10 percent, like the side of a measuring cup, filling as the day's food is logged. Budget and macro bars use it. Everything around it stays quiet.

**Sources are labeled.** Every estimated number says where it came from, in words. "Database" in blue means a matched food or product. "AI estimate" in berry means the model's guess. Color supports the label and is never the only signal.

## Theme

- Light and dark are defined once with CSS `light-dark()`. `color-scheme` picks the side.
- **System** is the default and follows `prefers-color-scheme`. **Light** and **Dark** set `<html data-theme>`.
- The choice lives in Settings, Appearance and is stored per device in `localStorage` under `cupful-theme`, wrapped in try/catch (`src/lib/theme.ts`).
- A small inline script in `app.html` applies the choice before first paint, so there is no flash.
- Text and muted text meet WCAG AA (4.5:1) on `--bg`, `--surface` and `--surface-2` in both themes. Semantic text meets AA on its soft fill. `--control-border` meets 3:1 on surfaces for input outlines.

## Palette

| Role | Light | Dark |
| --- | --- | --- |
| Page `--bg` | `#f2f5f6` | `#0e1418` |
| Card `--surface` | `#ffffff` | `#151d22` |
| Sunken `--surface-2` | `#e7edef` | `#1b262c` |
| Hairline `--border` | `#dce5e8` | `#2a3740` |
| Strong rule `--border-strong` | `#9aabb3` | `#60747f` |
| Control outline `--control-border` | `#8598a1` | `#60747f` |
| Text `--text` | `#172025` | `#e9f1f4` |
| Muted `--text-muted` | `#59666d` | `#9fb0b8` |
| Accent `--accent` | `#245f8a` | `#82c2e8` |
| Accent hover `--accent-hover` | `#1c4d70` | `#9ad1ef` |
| Accent soft `--accent-soft` | `#e1eef6` | `#173647` |
| On accent `--on-accent` | `#ffffff` | `#071820` |
| Under `--under` | `#2f7a6b` | `#8bd3c0` |
| Over `--over` | `#a64a52` | `#e39aa8` |
| Warning `--warning` | `#8a5a00` | `#e0b45a` |
| Error `--error` | `#b3261e` | `#e39aa8` |

### Data hues

| Meaning | Light | Dark |
| --- | --- | --- |
| Protein `--protein` | `#2c6e9e` | `#8fc3e6` |
| Carbs `--carbs` | `#9a651c` | `#e0b45a` |
| Fat `--fat` | `#a64a52` | `#e39aa8` |
| Steps `--steps` | `#2f7a6b` | `#8bd3c0` |
| Weight `--weight` | `#8e4f6b` | `#d8a6c8` |
| Breakfast `--breakfast` | `#c26a2a` | `#e8a06a` |
| Lunch `--lunch` | `#2f7a6b` | `#8bd3c0` |
| Dinner `--dinner` | `#3a5f8e` | `#8fb3e0` |
| Snack `--snack` | `#8a6a1f` | `#e0c078` |

Macros use `--protein`, `--carbs` and `--fat`, with `--*-text` for labels. Meal types use `--meal-{breakfast,lunch,dinner,snack}` plus `-soft` and `-text`, taken from the meal hues above. The Meals group headers use them with a dot and label, and the selected state of the meal-type `.segmented` control uses them.

Charts use `--chart-1..4`: protein, fat, steps, carbs. Intake bars use `--chart-intake` (blue). Bars over budget use `--chart-over` (brick). Weight uses `--chart-weight` (berry). Steps and rolling averages use `--chart-steps` and `--chart-average` (teal).

### Aliases

The old names keep working. `--coral` maps to fat, `--sage` to steps, `--gold` to snack, each with `-soft` and `-text`. `--sand` maps to `--surface-2` and `--sand-soft` to `--surface`. `.btn-coral` and `.sheet-coral` use the coral aliases, so the products area keeps its own tone. `--chart-*` map onto the data hues.

Meaning is separate from decoration. `--over` and `--error` are brick, distinct from the orange breakfast hue. `--under` is teal. `--warning` is amber. Meaning is always also in words.

Sources: `--src-db` is blue with a pale blue fill. `--src-ai` is berry with a pale berry fill. Both meet AA in both themes.

## Type

One family: **Manrope**, by the Manrope Project Authors, under the SIL Open Font License 1.1. The license is at `web/static/fonts/OFL.txt`.

- Files: `web/static/fonts/manrope-latin.woff2` and `manrope-latin-ext.woff2`. They are variable woff2 files covering weights 400 to 600.
- `@font-face` lives in `global.css`, uses `font-display: swap`, and points at local `/fonts/` paths. There is no Google Fonts request at runtime and the font is available offline after the first load.
- `--font-sans` and `--font-numeric` are both Manrope. Do not add a second typeface.
- Numbers use tabular figures through `font-variant-numeric: tabular-nums` and `font-feature-settings: 'tnum' 1` on `.num`.
- Sizes: h1 28 px (`--text-xl`), h2 20 px (`--text-lg`), body 16 px (`--text-md`), small 14 px (`--text-sm`), lead 44 px (`--text-hero`). `--text-2xl` is 32 px for the wordmark and large figures.

## Shape and surface

- Cards use `--radius-lg`, 20 px.
- Buttons, inputs and other controls use `--radius-md`, 12 px.
- Sheets and dialogs use `--radius-sheet`, 24 px.
- Chips use `--radius-pill`. Segmented options use `--radius-sm`, 10 px.
- Resting cards have a border and no shadow. `--shadow-card` is `none`.
- Only sheets, dialogs, popovers and the preview bar may use `--shadow-sheet`.
- There is no page gradient. The only gradient is the meter's graduation marks.

## Shell

- From 48 rem the top bar is sticky. It holds the wordmark, four plain links (Today, Activity, Trends, Settings) and a user chip.
- Brand: use the shipped logo set, not styled text. `web/src/lib/Wordmark.svelte` renders the light and dark wordmark SVGs and picks one from the active theme. Its colors match `--accent` and `--gold`. Favicons live in `web/static/favicon.svg`, `web/static/favicon.ico` and `web/static/icons/`.
- Below 48 rem the top bar becomes fixed bottom tabs with an icon and a short label per tab.
- The current route uses `--surface-2` with `--text`. It is never an accent fill or accent text.
- Every tab is at least 44 px tall. The bar adds `env(safe-area-inset-bottom)` on phones. The page reserves room for the bar.
- The shell works at 360 px with no sideways scroll, and at desktop widths.

## Motion

- Tokens: `--dur-fast` 120 ms, `--dur-med` 180 ms, `--dur-slow` 240 ms, `--ease` `cubic-bezier(0.2, 0.7, 0.2, 1)`.
- Feedback answers an action. A pressed button scales to 0.98. Hover changes surface tone. Focus draws a 2 px accent ring with a 2 px offset.
- Screens do not use entrance animations, except Trends. Its bars, cells, area fill and headline figures answer opening the screen and changing the range, in one short pass. Meters may fill after a save. Sheets may open.
- A global `prefers-reduced-motion` rule sets durations to near zero and disables smooth scrolling. The duration tokens also become 0. Skeleton pulses stop.

## Charts

Trends draws its own small SVG charts (`web/src/lib/trends/Charts.svelte`) so the line can stay continuous and the motion can be CSS. `ChartCard` frames every chart: title, headline figure, summary, readout and empty state.

- Intake bars use `--chart-intake` (blue). Bars over budget use `--chart-over` (brick). The budget line uses `--chart-budget` as a dashed path, broken on days without a budget.
- Weight uses `--chart-weight` (berry) for the line and its 14% area fill; the readout still names the 7-day average. Steps use `--chart-steps` (teal). Grid lines use `--chart-grid` and axis text uses `--chart-axis`.
- Color is never alone. A legend names each mark with a shape and a word. Over-budget days also say "over" in the readout.
- There is no floating tooltip, because it moves under the finger. The day touched, hovered or clicked is written as a sentence in a fixed two-line readout above the chart, for example "Fri 16 Oct: 1797 kcal eaten of 2154 kcal, 357 left." The sentence stays after the finger lifts, so tapping never shifts the layout.
- The plot is `role="img"` with an `aria-label` summary of range, counts and averages. Keyboard users get the summary only. There is no keyboard day-stepping yet.
- Each chart has its own `.empty` sentence that says what to log to fill it. The average card does the same. Gaps, such as no meals or no weigh-in, are gaps, never zeros.
- The range control (7d, 30d, 90d, 365d) is a `.segmented` control remembered per device. Bars thin to hairlines past 120 days.
- The weight line has a 14% area fill and no dash animation, so it reads as one continuous trend. Motion is 12 ms bar stagger, a fading area, popping consistency cells and growing macro/mix bars. Headline numbers count up over 700 ms with a single rAF helper. Reduced motion removes all of it.
- Consistency shows one cell per day, capped at 30 columns. Blue is within budget, brick is over, grey is no log.

## Activity

- The day's cards (Steps, Gym, Weigh-in) each carry one or two plain facts under the figure: steps as a `.meter` against the typical day and a 7-day average; gym sessions in the last 7 days and the last day trained; the weigh-in change since the previous one and a 30-day line. Facts only appear when the data exists. From 46 rem the three cards stretch to one height, with each caption pinned to the bottom. Gym's body parts are one "Trained: Chest, Legs" button that opens a checklist panel (a `<details class="menu">`, `dismiss.ts`: Esc, outside click, Done); each tick saves at once.
- "This week" is a full-width card under them: seven buttons ending on the selected day. Each shows steps as a `--steps` bar scaled to the week's highest, a dashed line at the typical day, a `--accent` dot on gym days, the synced workout count in a small pill on a fixed row (fits counts up to two digits), and the weekday and date. A day without steps is a thin tick, never a zero bar. Tapping a day selects it (the strip then ends on it). Each button's `aria-label` is a sentence ("Yesterday: 9,100 steps, gym, 1 workout"); the selected day has `aria-current="date"`. It fits 360 px with seven equal columns.
- "Workouts" lists what Google Health synced for the selected day: icon, name, time, duration, km, kcal, average heart rate, and a "Counts as gym" chip (`--accent-soft`), in two-line rows (name and time, then type and figures). Beyond six a day shows "Show N more". Distances are km because there is no distance unit setting. Empty: "No workouts synced for this day.", plus a Settings pointer only when Google is configured but not connected.
- The Budget card's "How it's worked out" is a small popover above its link (`.menu-panel.up`): one label/value line per server term in small tabular type, a Close button, Esc and outside click dismiss. It never pushes the page.
- A compact card reports what steps and gym added to the budget, from the server's terms.

## Tokens

Tokens live in `web/src/lib/styles/tokens.css`.

| Group | Tokens |
| --- | --- |
| Surfaces, text | `--bg`, `--surface`, `--surface-2` (sunken), `--border` (decorative), `--border-strong` (separators), `--control-border` (input outlines, 3:1), `--text`, `--text-muted` |
| Brand | `--accent`, `--accent-hover`, `--on-accent`, `--accent-soft`, `--focus-ring` |
| Meaning | `--under`/`--under-soft`, `--over`/`--over-soft`, `--warning`/`--warning-soft`, `--error`/`--error-soft` |
| Data | `--protein`, `--carbs`, `--fat`, `--steps`, `--weight`, `--breakfast`, `--lunch`, `--dinner`, `--snack`, each with text or soft variants where used |
| Aliases | `--coral`, `--sage`, `--gold` plus `-soft` and `-text`; `--sand`, `--sand-soft`; `--meal-*`; `--chart-1..4` |
| Sources | `--src-db`/`--src-db-soft` ("Database"), `--src-ai`/`--src-ai-soft` ("AI estimate") |
| Charts | `--chart-intake`, `--chart-budget`, `--chart-weight`, `--chart-average`, `--chart-steps`, `--chart-over`, `--chart-grid`, `--chart-axis` |
| Type | `--font-sans`, `--font-numeric`; `--text-xs` 13, `--text-sm` 14, `--text-md` 16, `--text-lg` 20, `--text-xl` 28, `--text-2xl` 32, `--text-hero` 44 px; `--leading-tight`, `--leading`; `--weight-regular` 400, `--weight-medium` 500, `--weight-bold` 600 |
| Space | `--space-1` through `--space-8` (4, 8, 12, 16, 24, 32, 40, 48 px), `--gutter`, `--content-max` (40 rem), `--page-max` (72 rem), `--touch` (44 px) |
| Shape | `--radius-sm` (10 px), `--radius-md` (12 px controls), `--radius-lg` (20 px cards), `--radius-sheet` (24 px), `--radius-pill`, `--shadow-card` (none), `--shadow-sheet` |
| Motion | `--dur-fast` 120, `--dur-med` 180, `--dur-slow` 240 ms, `--ease`. All durations are 0 under `prefers-reduced-motion` |

## Global classes

Global classes live in `web/src/lib/styles/global.css`. Screens use them and add scoped styles only for layout. There is no CSS framework, UI kit or icon pack. Icons are a few inline SVG paths using `currentColor`.

- Layout: `.stack`, `.row`.
- Text: `.num` (tabular numerals), `.muted`, `.small`, `.visually-hidden`.
- Cards: `.card`.
- Buttons: `.btn` with `.btn-primary`, `.btn-quiet` or `.btn-danger`, and `.btn-block`.
- Forms: `.field` (label, control, `.hint`). Native inputs are styled globally. Checkboxes and radios are drawn from tokens, not `accent-color`, so they match both themes. Invalid fields use `[aria-invalid='true']`.
- `.segmented` turns a radio group into a segmented control. The selected option gets a `--surface` fill, normal `--text` and bold text. Meal types replace that with their soft fill and text hue.
- Lists and chips: `.list` (rows with dividers), `.chip` with `.chip-db`, `.chip-ai`, `.chip-warning`, `.chip-error` or `.chip-over`.
- Meters: `.meter` with `style="--value: 0.62; --fill: var(--protein)"`. Add `.is-over` past 1.
- States: `.notice` with `.notice-error`, `.notice-warning` or `.notice-info`; `.empty`; `.skeleton`.
- Sheets: `dialog.sheet` is a native `<dialog>` shown as a bottom sheet on phones and centered on wide screens. It scrolls itself. Its title row is a `<header class="sheet-head">` that stays in view while the sheet scrolls.

## Component rules

- Touch targets are at least 44 by 44 px (`--touch`). Inputs use 16 px text so iOS does not zoom.
- The app works at 360 px wide with no sideways scroll; the e2e test checks this. Phones get a bottom tab bar. From 48 rem there is a top bar. From 60 rem Today splits into budget beside logging and meals.
- Focus is always visible: a 2 px `--focus-ring` outline via `:focus-visible`. Never remove it without a replacement.
- Every control has a label: a visible `<label>`, or `.visually-hidden` when the context is obvious, such as the date picker. Icon-only buttons get `aria-label`.
- Color is never the only signal. "Over" says "over", sources say their name, flags are words.
- Errors use a `.notice-error` with `role="alert"`, next to what failed. The message says what happened and what to do next, in plain words (`errorMessage()` in `src/lib/api.ts`). Errors do not apologize. When the fix is in Settings, the message links there.
- Empty states use `.empty` with one sentence inviting the next action, such as "Nothing logged yet. Describe your breakfast above.", and optionally one button.
- Loading uses `.skeleton` blocks the size of what is coming, with `aria-busy="true"` on the container. No spinners for waits under a second.
- Motion answers an action, such as a sheet opening or a meter filling after a save, and uses the duration tokens. Reduced motion turns it off. There are no entrance animations.
- Disclosures and pickers use a quiet `.btn` with `aria-expanded` to reveal extra inputs ("Edit numbers") or an inline panel on `--surface-2` ("Use my product": a search field and a `.list` of buttons) inside the item card. There are no nested dialogs.
- Copy uses sentence case and plain verbs. Buttons say what happens ("Save meal", "Scan barcode"), and an action keeps its name through the flow.
