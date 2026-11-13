<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Design

People open Cupful many times a day, on a phone, for a few seconds. They check what is left, log what they ate, and put the phone away. The design serves that glance.

## Direction

**Ember jug.** The budget is a measuring jug: you pour the day's food into it and see how much room is left. The palette is graphite and ember. The light theme sits on cool `#f1f3f6` with `#fafbfc` panels, so it does not glare. The dark theme sits on `#0a0d10` with `#12161b` panels. The accent is a burnt ember: `#c2410c` in light, and a softened `#d9824f` in dark, because the brighter `#fb8b3c` read harsh on graphite. A faint ember wash at the top left and blue wash at the top right sit behind the page (`body::before`, fixed, about 300 px, decorative).

**Pantry layout.** Soft surfaces, generous spacing and large radii. Cards sit on the ground with a hairline border and no shadow. Desktop gets a top bar with the wordmark, four plain links and an account menu. Phones get bottom tabs with an icon and a short label. The current route is neutral: `--surface-2` with normal text on desktop, and `--accent-text` on the phone tabs.

Nothing shouts. Strong colors carry meaning. Teal means "left". Rose means "over". Amber means "check this". Data hues: blue protein, ochre carbs, rose fat, teal steps, berry weight, and ember, teal, blue and gold for the four meals.

**Numbers first.** Figures use Bricolage Grotesque (`.fig`, 800 weight) with tabular figures, so totals do not jitter as they change. The numbers people glance at are large: 28 px for a page title and a 56 px kcal-left figure on the Today jug card.

**The jug.** The signature element is the measuring jug: an open-top outline in `--text`, graduations, and a liquid (`--fill`, with a lighter `--fill-top` meniscus) whose height is the share of the plan eaten. The liquid is decorative: it measures 2.2:1 against `--surface` in light, so the jug outline (`--text`, over 16:1) carries the 3:1 for the shape and the figures carry the meaning in text. The jug is `role="img"` with an `aria-label`. `.meter` is the same idea in small: a track with graduations every 10 percent and a fill.

**Sources are labeled.** Every estimated number says where it came from, in words. "Database" in blue means a matched food or product. "AI estimate" in berry means the model's guess. Color supports the label and is never the only signal.

## Theme

- Light and dark are defined once with CSS `light-dark()`. `color-scheme` picks the side.
- **System** is the default and follows `prefers-color-scheme`. **Light** and **Dark** set `<html data-theme>`.
- The choice lives in Settings, Appearance and is stored per device in `localStorage` under `cupful-theme`, wrapped in try/catch (`src/lib/theme.ts`).
- A small inline script in `app.html` applies the choice before first paint, so there is no flash.
- Text and muted text meet WCAG AA (4.5:1) on `--bg`, `--surface` and `--surface-2` in both themes. Semantic text meets AA on its soft fill. `--control-border` meets 3:1 on surfaces for input outlines.

## Palette

Ratios are WCAG contrast, light / dark, measured with a script over the token values.

| Role | Light | Dark | Pair and ratio |
| --- | --- | --- | --- |
| Page `--bg` | `#f1f3f6` | `#0a0d10` | |
| Card `--surface` | `#fafbfc` | `#12161b` | |
| Sunken `--surface-2` | `#e6e9ee` | `#1b2027` | |
| Hairline `--border` | `#dde1e7` | `#232a32` | decorative |
| Strong rule `--border-strong` | `#a9b1bb` | `#47515c` | separators |
| Control outline `--control-border` | `#8a929c` | `#66717d` | on surface 3.04 / 3.65 |
| Text `--text` | `#12161b` | `#edf1f5` | on bg 16.34 / 17.16, on surface 17.53 / 16.00, on surface-2 14.92 / 14.43 |
| Muted `--text-muted` | `#59616b` | `#96a1ad` | on bg 5.64 / 7.42, on surface 6.06 / 6.92, on surface-2 5.16 / 6.24 |
| Accent `--accent` | `#c2410c` | `#d9824f` | on bg 4.66 / 6.73, on surface 5.00 / 6.28 |
| Accent hover `--accent-hover` | `#a8380a` | `#e39264` | |
| On accent `--on-accent` | `#fff8f3` | `#1c0d05` | on accent 4.93 / 6.54, on hover 6.17 / 7.72 |
| Accent soft `--accent-soft` | `#fbe7da` | `#2a1d15` | |
| Accent text `--accent-text` | `#9a330a` | `#eba783` | on accent-soft 6.15 / 8.08, on surface 7.11 / 8.98 |
| Under `--under` / `--under-soft` | `#0e7862` / `#d9f2ea` | `#4fd1b0` / `#0f2a24` | 4.60 / 8.05 |
| Over `--over` / `--over-soft` | `#ba3858` / `#fbe3e9` | `#e48aa0` / `#2e1820` | 4.55 / 6.66 |
| Warning `--warning` / `--warning-soft` | `#8a5a00` / `#f0eade` | `#e0b45a` / `#2f2a1b` | 4.95 / 7.39 |
| Error `--error` / `--error-soft` | `#b3261e` / `#f5e3e2` | `#e48aa0` / `#381f21` | 5.28 / 6.82 |
| Jug liquid `--fill`, `--fill-top` | `#f2955a`, `#f9bf98` | `#c9743f`, `#dd9a70` | decorative; `--fill` on surface 2.20 / 5.24 |
| Empty jug `--glass` | `rgb(43 99 196 / 0.05)` | `rgb(127 176 255 / 0.05)` | |

Two fixes against the first pass of the plan: light `--under` on `--under-soft` measured 4.48:1 and light `--over` on `--over-soft` 4.38:1, so the inks were darkened from `#0f7a63` to `#0e7862` and from `#be3a5a` to `#ba3858` (the fills stay). `--accent` on `--accent-soft` is 4.33:1 in light, so ember text on a soft fill uses `--accent-text`, never `--accent`.

### Data hues

| Meaning | Light | Dark | On `--surface` |
| --- | --- | --- | --- |
| Protein `--protein` | `#2b63c4` | `#7fb0ff` | 5.50 / 8.26 |
| Carbs `--carbs` | `#a16207` | `#fcd34d` | 4.75 / 12.60 |
| Fat `--fat` | `#be3a5a` | `#fb93ad` | 5.14 / 8.54 |
| Steps `--steps` | `var(--under)` | `var(--under)` | 5.22 / 9.58 |
| Weight `--weight` | `#8e4f6b` | `#d8a6c8` | 5.82 / 8.83 |
| Breakfast `--breakfast` | `#c2410c` | `#d9824f` | 5.00 / 6.28 |
| Lunch `--lunch` | `var(--under)` | `var(--under)` | 5.22 / 9.58 |
| Dinner `--dinner` | `#2b63c4` | `#7fb0ff` | 5.50 / 8.26 |
| Snack `--snack` | `#8a6a1f` | `#e7c76a` | 4.87 / 11.05 |
| Source DB `--src-db` / soft | `#2b63c4` / `#e2eafa` | `#7fb0ff` / `#142338` | 4.72 / 7.20 on soft |
| Source AI `--src-ai` / soft | `#8e4f6b` / `#f0e8ec` | `#d8a6c8` / `#302832` | 5.02 / 6.92 on soft |

Macros use `--protein`, `--carbs` and `--fat`, with `--*-text` for labels. Meal types use `--meal-{breakfast,lunch,dinner,snack}` plus `-soft` and `-text`. The Meals group headers use them with a dot and label, and the selected state of the meal-type `.segmented` control uses them.

Charts use `--chart-1..4`: protein, fat, steps, carbs. Intake bars use `--chart-intake` (blue). Bars over budget use `--chart-over` (rose). Weight uses `--chart-weight` (berry). Steps and rolling averages use `--chart-steps` and `--chart-average` (teal).

### Aliases

The old names keep working. `--coral` maps to over, `--sage` to under, `--gold` to snack, each with `-soft` and `-text`. `--sand` maps to `--surface-2` and `--sand-soft` to `--surface`. `.btn-coral` and `.sheet-coral` use the coral aliases, so the products area keeps its own tone. `--chart-*` map onto the data hues.

Meaning is separate from decoration. `--over` and `--error` are rose, distinct from the ember breakfast hue. `--under` is teal. `--warning` is amber. Meaning is always also in words.

## Type

Two families, both variable, self-hosted as woff2 under the SIL Open Font License 1.1.

- **Bricolage Grotesque** (display): figures, h1, h2, card titles and the wordmark. Axes kept: `opsz` 12 to 96 and `wght` 400 to 800 (`wdth` pinned to 100). Licence: `web/static/fonts/OFL-bricolage-grotesque.txt`.
- **Instrument Sans** (everything else): `wght` 400 to 700 (`wdth` pinned to 100). Licence: `web/static/fonts/OFL-instrument-sans.txt`.
- Files: `{bricolage-grotesque,instrument-sans}-{latin,latin-ext}.woff2` in `web/static/fonts/`, subset with `pyftsubset --flavor=woff2` from the upstream Google Fonts files. `@font-face` lives in `styles/fonts.css`, uses `font-display: swap` and local `/fonts/` paths. There is no font request to a third party, and the files are available offline after the first load. `app.html` preloads `instrument-sans-latin.woff2`.
- `--font-display` is Bricolage, `--font-sans` and `--font-numeric` are Instrument Sans. Do not add a third typeface.
- Numbers use tabular figures through `font-variant-numeric: tabular-nums` and `font-feature-settings: 'tnum' 1` on `.num`, and on `.fig`.
- Sizes: h1 28 px (`--text-xl`), h2 20 px (`--text-lg`), body 16 px (`--text-md`), small 14 px (`--text-sm`), lead 44 px (`--text-hero`). `--text-2xl` is 32 px for large figures.

## The mark

One logo, the measuring jug, everywhere: an open-top jug outline in ink with a pour lip at the top left, and an ember liquid at about 40 percent with a lighter meniscus line. The outline is a 2.6 unit stroke on a 30 by 32 grid, so it holds at 16 px.

- `web/src/lib/assets/mark.svg`: the mark alone, follows `prefers-color-scheme`.
- `web/src/lib/assets/wordmark-light.svg` and `wordmark-dark.svg`: mark plus "Cupful" in Bricolage Grotesque 800, converted to outline paths with fontTools, so they render without the font (for example in the README on GitHub). `Wordmark.svelte` shows the pair and is the only way the app shows the logo.
- `web/static/favicon.svg` (follows `prefers-color-scheme`), `favicon.ico` (16, 32 and 48 on the `#f1f3f6` ground), `icons/apple-touch-icon.png` (180).
- PWA icons: `icon-192.png` and `icon-512.png` (`purpose: any`, rounded ground), `maskable-192.png` and `maskable-512.png` (`purpose: maskable`, the mark is half the canvas height on a solid `#f1f3f6` ground, well inside the central 80 percent safe zone; checked under a circle mask).
- The manifest uses background `#f1f3f6` and theme `#c2410c`. `app.html` carries `theme-color` `#f1f3f6` for light and `#0a0d10` for dark.

## Shape and surface

- Cards use `--radius-lg`, 20 px.
- Buttons, inputs and other controls use `--radius-md`, 12 px.
- Sheets and dialogs use `--radius-sheet`, 24 px.
- Chips use `--radius-pill`. Segmented options use `--radius-sm`, 10 px.
- Resting cards have a border and no shadow. `--shadow-card` is `none`.
- Only sheets, dialogs, popovers and the preview bar may use `--shadow-sheet`.
- The only gradients are the meter's graduations and the faint page glow.

## Shell

- From 48 rem the top bar is sticky. It holds the wordmark, four plain links (Today, Activity, Trends, Settings) and an avatar. The avatar opens a menu with **Profile** and **Log out**.
- Brand: use the shipped logo set, not styled text (see The mark). `web/src/lib/Wordmark.svelte` renders the light and dark wordmark SVGs and picks one from the active theme.
- The date navigation is one right-aligned group: **Back to today** when needed, previous, date, next. The phone label shortens to **Today**. It never centers when the back button appears.
- Profile and account live at `/profile`, reached from the avatar menu on wide screens and from Settings on phones. Settings itself keeps the instance and device controls and uses the full page width.
- Below 48 rem the top bar becomes fixed bottom tabs with an icon and a short label per tab.
- The current route uses `--surface-2` with `--text` in the top bar and `--accent-text` on the phone tabs. It is never an accent fill.
- Every tab is at least 44 px tall. The bar adds `env(safe-area-inset-bottom)` on phones. The page reserves room for the bar.
- The shell works at 360 px with no sideways scroll, and at desktop widths.

## Motion

- One scale, as tokens: `--dur-fast` 140 ms for press, hover and focus; `--dur-med` 260 ms for state swaps, chevrons, chips and the view fade; `--dur-slow` 520 ms for meters, count-ups and growing bars; `--dur-pour` 820 ms for the jug fill only; `--dur-enter` 420 ms for sheets in; `--dur-exit` 180 ms for sheets and swaps out.
- Two curves: `--ease` `cubic-bezier(0.22, 1, 0.36, 1)` for anything entering or changing, and `--ease-in` `cubic-bezier(0.4, 0, 1, 1)` for exits. `--ease-state` is an alias of `--ease`.
- Feedback answers an action. A pressed `.btn` scales to 0.97 (no-preference only). Hover changes surface tone. Focus draws a 2 px accent ring with a 2 px offset.
- Movement settles without overshoot. Transform and opacity only; block-size via `interpolate-size` is the one exception.
- Helpers in `styles/motion.css`: `.grow-y`, `.grow-x`, `.pop` and `.rise`, stagger with `--n` (40 ms steps). They exist only inside `@media (prefers-reduced-motion: no-preference)`, so reduced motion renders the final state at once and nothing waits on script at opacity 0.
- `#lib/motion/CountUp.svelte` counts a number to its value over `--dur-slow`, from 0 on mount and from the number on screen whenever `value` changes. Under reduced motion it snaps.
- A global `prefers-reduced-motion` rule sets every duration token to 0 and disables smooth scrolling. Sheets close at once and skeletons stop pulsing.

## Charts

Trends draws its own small SVG charts (`web/src/lib/trends/Charts.svelte`) so the line can stay continuous and the motion can be CSS. `ChartCard` frames every chart: title, headline figure, summary, readout and empty state.

- Intake bars use `--chart-intake` (blue). Bars over budget use `--chart-over` (brick). The budget line uses `--chart-budget` as a dashed path, broken on days without a budget.
- Weight uses `--chart-weight` (berry) for the line and its 14% area fill; the readout still names the 7-day average. Steps use `--chart-steps` (teal). Grid lines use `--chart-grid` and axis text uses `--chart-axis`.
- Color is never alone. A legend names each mark with a shape and a word. Over-budget days also say "over" in the readout.
- There is no floating tooltip, because it moves under the finger. The day touched, hovered or clicked is written as a sentence in a fixed two-line readout above the chart, for example "Fri 16 Oct: 1797 kcal eaten of 2154 kcal, 357 left." The sentence stays after the finger lifts, so tapping never shifts the layout.
- The plot is `role="img"` with an `aria-label` summary of range, counts and averages. Keyboard users get the summary only. There is no keyboard day-stepping yet.
- Each chart has its own `.empty` sentence that says what to log to fill it. The average card does the same. Gaps, such as no meals or no weigh-in, are gaps, never zeros.
- The range control (7d, 30d, 90d, 365d) is a `.segmented` control remembered per device. Bars thin to hairlines past 120 days.
- The weight line has a 14% area fill and no dash animation, so it reads as one continuous trend. Motion is 760 ms bar growth with a 16 ms stagger capped at 15 bars, an area fade over 600 ms after 120 ms, consistency cells that fade over 360 ms in 18 ms steps instead of popping, and macro/mix bars growing over 760 ms in 80 ms steps. Headline numbers count with `CountUp` over `--dur-slow`. Reduced motion removes all of it.
- Consistency shows one cell per day, capped at 30 columns. Blue is within budget, brick is over, grey is no log.
- "Eaten most" lists the five most-logged meals in the range: label, meal type, count, last date and average kcal, with a thin `--accent` bar scaled to the top count. It hides when the server omits `top_meals` and shows an empty sentence for an empty range. Only rows present when the range mounted animate.

## Activity

- The day's cards (Steps, Gym, Weigh-in) each carry one or two plain facts under the figure: steps as a `.meter` against the typical day and a 7-day average; gym sessions in the last 7 days and the last day trained; the weigh-in change since the previous one and a 30-day line. Facts only appear when the data exists. From 46 rem the three cards stretch to one height, with each caption pinned to the bottom. Gym's body parts are one "Trained: Chest, Legs" button that opens a checklist panel (a `<details class="menu">`, `dismiss.ts`: Esc, outside click, Done); each tick saves at once. When the day has no weigh-in, the card shows the latest earlier weigh-in with a "Carried" chip and the source date in the caption; editing it saves a weigh-in for the selected day.
- "This week" is a full-width card under them: a fixed seven-day window. Each day shows steps as a `--steps` bar scaled to the week's highest, a dashed line at the typical day, a `--accent` dot on gym days, the synced workout count in a small pill on a fixed row (fits counts up to two digits), and the weekday and date. A day without steps is a thin tick, never a zero bar. Tapping a day inside the window selects it without moving the window; only a date outside the window shifts it. Each button's `aria-label` is a sentence ("Yesterday: 9,100 steps, 40 min gym, Workout: Strength training"); hovering or focusing a day shows the same steps, gym, trained parts and workout names in a tooltip. The selected day has `aria-current="date"`. It fits 360 px with seven equal columns.
- Selecting a day keeps the current cards, budget, week strip and workouts on screen while the new day loads. Inputs are disabled until it arrives, so a date tap does not collapse the page into skeletons.
- "Workouts" lists what Google Health synced for the selected day: icon, name, time, duration, km, kcal, average heart rate, and a "Counts as gym" chip (`--accent-soft`), in two-line rows (name and time, then type and figures). Beyond six a day shows "Show N more". Distances are km because there is no distance unit setting. Empty: "No workouts synced for this day.", plus a Settings pointer only when Google is configured but not connected.
- The Budget card's "How it's worked out" is a small popover above its link (`.menu-panel.up`): one label/value line per server term in small tabular type, a Close button, Esc and outside click dismiss. It never pushes the page.
- A compact card reports what steps and gym added to the budget, from the server's terms. With a fixed goal it says "Burned beyond base" and shows the goal itself.

## Tokens

Tokens live in `web/src/lib/styles/tokens.css`.

| Group | Tokens |
| --- | --- |
| Surfaces, text | `--bg`, `--surface`, `--surface-2` (sunken), `--border` (decorative), `--border-strong` (separators), `--control-border` (input outlines, 3:1), `--text`, `--text-muted` |
| Brand | `--accent`, `--accent-hover`, `--on-accent`, `--accent-soft`, `--accent-text`, `--focus-ring` |
| Jug | `--fill`, `--fill-top`, `--glass` |
| Meaning | `--under`/`--under-soft`, `--over`/`--over-soft`, `--warning`/`--warning-soft`, `--error`/`--error-soft` |
| Data | `--protein`, `--carbs`, `--fat`, `--steps`, `--weight`, `--breakfast`, `--lunch`, `--dinner`, `--snack`, each with text or soft variants where used |
| Aliases | `--coral`, `--sage`, `--gold` plus `-soft` and `-text`; `--sand`, `--sand-soft`; `--meal-*`; `--chart-1..4` |
| Sources | `--src-db`/`--src-db-soft` ("Database"), `--src-ai`/`--src-ai-soft` ("AI estimate") |
| Charts | `--chart-intake`, `--chart-budget`, `--chart-weight`, `--chart-average`, `--chart-steps`, `--chart-over`, `--chart-grid`, `--chart-axis` |
| Type | `--font-display`, `--font-sans`, `--font-numeric`; `--text-xs` 13, `--text-sm` 14, `--text-md` 16, `--text-lg` 20, `--text-xl` 28, `--text-2xl` 32, `--text-hero` 44 px; `--leading-tight`, `--leading`; `--weight-regular` 400, `--weight-medium` 500, `--weight-bold` 600 |
| Space | `--space-1` through `--space-8` (4, 8, 12, 16, 24, 32, 40, 48 px), `--gutter`, `--content-max` (40 rem), `--page-max` (72 rem), `--touch` (44 px) |
| Shape | `--radius-sm` (10 px), `--radius-md` (12 px controls), `--radius-lg` (20 px cards), `--radius-sheet` (24 px), `--radius-pill`, `--shadow-card` (none), `--shadow-sheet` |
| Motion | `--dur-fast` 140, `--dur-med` 260, `--dur-slow` 520, `--dur-pour` 820, `--dur-enter` 420, `--dur-exit` 180 ms; `--ease`, `--ease-in`, `--ease-state`. All durations are 0 under `prefers-reduced-motion` |

## Global classes

Global classes live in `web/src/lib/styles/`, split by behaviour (`base`, `buttons`, `forms`, `segmented`, `components`, `sheet`, `motion`) and imported from `global.css`. Screens use them and add scoped styles only for layout. There is no CSS framework, UI kit or icon pack. Icons are a few inline SVG paths using `currentColor`.

- Layout: `.stack`, `.row`.
- Text: `.num` (tabular numerals), `.muted`, `.small`, `.visually-hidden`.
- Cards: `.card`, `.card-head` (an `h2` and a muted right-hand note), `.tile` (a compact stat card, or a link with `a.tile`), `.fig` (big figure, with a muted `small` unit).
- Buttons: `.btn` with `.btn-primary`, `.btn-soft`, `.btn-quiet` or `.btn-danger`, and `.btn-block`.
- Forms: `.field` (label, control, `.hint`). Native inputs are styled globally. Checkboxes and radios are drawn from tokens, not `accent-color`, so they match both themes. Invalid fields use `[aria-invalid='true']`.
- `.seg` is the same control built from buttons with `aria-pressed`. `.segmented` turns a radio group into a segmented control. The selected option gets a `--surface` fill, normal `--text` and bold text. Meal types replace that with their soft fill and text hue.
- Lists and chips: `.list` (rows with dividers), `.chip` with `.chip-db`, `.chip-ai`, `.chip-warning`, `.chip-error`, `.chip-over`, `.chip-under` or `.chip-accent`. `.rows` is a divided list of label and value rows with `small` sublines.
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
- Motion answers an action, such as a sheet opening or a meter filling after a save, and uses the duration tokens. Reduced motion turns it off. There are no entrance animations beyond Trends and sheets.
- Repeat is review-first. **Repeat a meal** in the composer opens a recent-meals sheet: the last year's distinct meals, searchable by name or ingredient, up to 50 groups. Picking one opens Review with a copy for the selected day. Cancelling Review returns to the sheet with the search intact; saving leaves the composer draft untouched. On a logged meal, **Repeat** logs another on the selected day; on a past day it reads **Copy to today**, saves to today, and offers a **View today** notice. Nothing is scheduled for the future.
- Disclosures and pickers use a quiet `.btn` with `aria-expanded` to reveal extra inputs ("Edit numbers") or an inline panel on `--surface-2` ("Use my product": a search field and a `.list` of buttons) inside the item card. There are no nested dialogs.
- Copy uses sentence case and plain verbs. Buttons say what happens ("Save meal", "Scan barcode"), and an action keeps its name through the flow.
