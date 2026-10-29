# Design

Cupful is opened many times a day, on a phone, for a few seconds at a time: check what's left, log what was eaten, put the phone away. The design serves that glance.

## Direction

**Calm, not clinical.** Brand blue is primary (links, focus, primary buttons); coral, sage and sand-gold sit beside it for categories (meal types, macros, the products area). The light theme sits on a warm sand tint, the dark theme on deep blue (not black). Nothing shouts; the strong colors are meaning: emerald for "left", crimson for "over", amber-orange for "check this". The owner chose the blue wellness theme, then asked for coral, sage and sand to complement it.

**Numbers first.** The numbers people glance at (kcal left, macro grams, totals) use the system's rounded face (`ui-rounded`, SF Pro Rounded on Apple devices, falling back to the system sans) with tabular digits, so totals don't jitter as they change. Everything else is the plain system sans. No web fonts: nothing to download, and the PWA works offline.

**The measuring cup.** The one signature element is the `.meter`: a track with graduation marks every 10 %, like the side of a measuring cup, filling as the day's food is logged. Budget and macro bars use it. Everything around it stays quiet: one card style, hairline borders, a soft blue shadow.

**Sources are labeled.** Every estimated number says where it came from, in words: "Database" (cyan-teal) for matched foods and products, "AI estimate" (violet) for the model's guess. Color supports the label; it is never the only signal.

## Theme

- Light and dark are defined once with CSS `light-dark()`; `color-scheme` picks the side.
- **System** (default) follows `prefers-color-scheme`. **Light**/**Dark** set `<html data-theme>`.
- The choice lives in Settings → Appearance, stored per device in `localStorage` (`cupful-theme`, wrapped in try/catch; `src/lib/theme.ts`). A small inline script in `app.html` applies it before first paint, so there's no flash.
- All text tokens meet WCAG AA (4.5:1) on `--bg`, `--surface` and `--surface-2` in both themes, including over/warning/error and the source labels on their soft backgrounds; meter, chart and input-border colors meet 3:1 against surfaces.

## Palette

| Role | Light / dark mark | Soft | Text |
|---|---|---|---|
| Brand blue `--accent` | `#2b67a3` / `#84b8ea` | `--accent-soft` | `--accent` |
| Coral `--coral` | `#cb5a3e` / `#ee8a70` | `#fbe9e3` / `#4a2a33` | `--coral-text` `#a8402a` / `#f5a891` |
| Sage `--sage` | `#4f8a6b` / `#7fc09f` | `#e3f0e8` / `#1b3a2c` | `--sage-text` `#2f6a4d` / `#98d3b3` |
| Sand-gold `--gold` | `#9a7710` / `#e2bd5a` | `#f6edd0` / `#3a3220` | `--gold-text` `#6e5410` / `#ecca80` |
| Sand `--sand`, `--sand-soft` | `#efe8da`, `#f8f4ec` / `#22344a`, `#1a2b3d` | page and sunken surfaces (light) | |

- **Meal types**: `--meal-{breakfast,lunch,dinner,snack}` (+ `-soft`, `-text`) = coral, sage, blue, sand-gold. Used by the Meals group headers (dot + label) and the selected state of the meal-type `.segmented` control.
- **Macros**: `--protein` blue, `--carbs` sage, `--fat` coral (+ `--*-text` for labels). **Charts** (M4): `--chart-1..4` = blue, coral, sage, gold.
- **Coral** marks the products area: `.btn-coral` ("Scan barcode", "Saved products", "Use my product"), the product sheet title (`.sheet-coral`), the picker panel.
- **Meaning is separate from decoration**: `--over`/`--error` crimson (deeper and pinker than coral), `--under` emerald (bluer than sage), `--warning` amber-orange (redder than gold). Meaning is always also in words.
- Rule: blue stays primary; aim for 2-3 accents per screen; accent colors are for categories, never for state.

## Tokens (`web/src/lib/styles/tokens.css`)

| Group | Tokens |
|---|---|
| Surfaces, text | `--bg`, `--surface`, `--surface-2` (sunken), `--border` (decorative), `--border-strong` (inputs, 3:1), `--text`, `--text-muted` |
| Brand | `--accent`, `--accent-hover`, `--on-accent`, `--accent-soft`, `--focus-ring` |
| Meaning | `--under`/`--under-soft` (left, success), `--over`/`--over-soft`, `--warning`/`--warning-soft`, `--error`/`--error-soft` |
| Palette | `--coral`, `--sage`, `--gold` (+ `-soft`, `-text`), `--sand`, `--sand-soft`, `--meal-*`, `--protein-text`/`--carbs-text`/`--fat-text`, `--chart-1..4` |
| Sources | `--src-db`/`--src-db-soft` ("Database"), `--src-ai`/`--src-ai-soft` ("AI estimate") |
| Macros, charts | `--protein`, `--carbs`, `--fat`, `--chart-intake`, `--chart-budget`, `--chart-weight`, `--chart-average`, `--chart-steps`, `--chart-over`, `--chart-grid`, `--chart-axis` |
| Type | `--font-sans`, `--font-numeric`; `--text-xs` 13 · `--text-sm` 14 · `--text-md` 16 · `--text-lg` 20 · `--text-xl` 25 · `--text-2xl` 32 · `--text-hero` 52 px; `--leading-tight`, `--leading`; `--weight-regular/medium/bold` |
| Space | `--space-1`…`--space-8` (4, 8, 12, 16, 24, 32, 40, 48 px), `--gutter`, `--content-max` (40rem), `--touch` (44 px) |
| Shape | `--radius-sm` (chips), `--radius-md` (buttons, inputs), `--radius-lg` (cards), `--radius-sheet` (dialogs), `--radius-pill`; `--shadow-card`, `--shadow-sheet` |
| Motion | `--dur-fast` 120, `--dur-med` 220, `--dur-slow` 420 ms, `--ease`; all 0 under `prefers-reduced-motion` |

## Global classes (`web/src/lib/styles/global.css`)

Screens use these and add scoped styles only for layout. No CSS framework, UI kit or icon pack; icons are a few inline SVG paths using `currentColor`.

- Layout: `.stack`, `.row`. Text: `.num` (rounded tabular numerals), `.muted`, `.small`, `.visually-hidden`.
- `.card`. Buttons: `.btn` + `.btn-primary` / `.btn-quiet` / `.btn-danger`, `.btn-block`.
- Forms: `.field` (label + control + `.hint`), native inputs styled globally (checkboxes and radios are drawn from tokens, not `accent-color`, so they match both themes), `[aria-invalid='true']`, `.segmented` (radio group as a segmented control; the selected option gets `--accent-soft`, an `--accent` outline and bold text).
- `.list` (rows with dividers), `.chip` + `.chip-db` / `.chip-ai` / `.chip-warning` / `.chip-over`.
- `.meter` with `style="--value: 0.62; --fill: var(--protein)"`; add `.is-over` past 1.
- States: `.notice` + `.notice-error` / `.notice-warning` / `.notice-info`, `.empty`, `.skeleton`.
- `dialog.sheet`: a native `<dialog>` as a bottom sheet on phones, centered on wide screens; it scrolls itself. Its title row is a `<header class="sheet-head">` that stays in view while the sheet scrolls.

## Component rules

- **Touch targets** at least 44 × 44 px (`--touch`). Inputs are 16 px text so iOS doesn't zoom.
- **Works at 360 px wide** with no sideways scroll (the e2e test checks). Phones get a bottom tab bar; from 48rem a top bar; from 60rem Today splits into budget beside logging and meals.
- **Focus** is always visible: a 3 px `--focus-ring` outline via `:focus-visible`. Never remove it without a replacement.
- **Labels** on every control (a visible `<label>`, or `.visually-hidden` when the context is obvious, like the date picker). Icon-only buttons get `aria-label`.
- **Color is never the only signal**: "over" says "over", sources say their name, flags are words.
- **Errors**: a `.notice-error` with `role="alert"`, next to what failed, saying what happened and what to do next, in plain words (`errorMessage()` in `src/lib/api.ts`). Errors don't apologize. When the fix is in Settings (no provider, bad key), link there.
- **Empty**: `.empty` with one sentence inviting the next action ("Nothing logged yet. Describe your breakfast above."), optionally one button.
- **Loading**: `.skeleton` blocks the size of what's coming, with `aria-busy="true"` on the container. No spinners for under a second.
- **Motion** answers an action (a sheet opening, a meter filling after a save) and uses the duration tokens, so reduced-motion turns it off. No entrance animations.
- **Disclosures and pickers** (review sheet): a quiet `.btn` with `aria-expanded` reveals extra inputs ("Edit numbers") or an inline panel on `--surface-2` ("Use my product": search field plus a `.list` of buttons) inside the item card; no nested dialogs.
- **Copy**: sentence case, plain verbs, buttons say what happens ("Save meal", "Scan barcode"), and an action keeps its name through the flow.
