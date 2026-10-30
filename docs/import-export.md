# Import and export

Cupful exchanges its food log as CSV: `GET /api/export.csv` writes every day, meal and item, and `POST /api/import/fuel-ledger` reads the same layout. The format is compatible with Fuel Ledger, so it is a way to move a food log in or out, not a way to clone a Cupful instance. For that, use the JSON backup in [`backups.md`](backups.md).

CSV files are tied to one account on one instance: exported `meal_id`s are Cupful UUIDs that name rows in one database. To move an account to another server or another user, use a JSON backup, not a CSV file.

## Endpoints

| Request | Auth | Body | Limits |
| --- | --- | --- | --- |
| `GET /api/export.csv` | session cookie | none | exports all history; there is no date filter |
| `POST /api/import/fuel-ledger` | session cookie and `X-Cupful: 1` | raw `text/csv` | 10 MB, 50,000 data rows (excluding the header) |

The import is one transaction: either the whole file is applied, or nothing is. A failed import leaves the account unchanged.

```sh
curl -b cookies.txt https://cupful.example.ts.net/api/export.csv -o cupful.csv

curl -b cookies.txt -X POST https://cupful.example.ts.net/api/import/fuel-ledger \
  -H 'X-Cupful: 1' \
  -H 'Content-Type: text/csv' \
  --data-binary @fuel-ledger.csv
```

The import answers `200` with a JSON summary. Errors are JSON with the line number in the message, for example `{"error": "line 42: unknown meal type \"brunch\""}`; a file over the row or byte limit is rejected before anything is written. Because a failed import rolls back, line numbers refer to the file as submitted.

## Export format

The first line is exactly this header, with all 16 columns:

```csv
date,meal_id,item_index,meal,description,food,portion,kcal,protein_g,carbs_g,fat_g,steps,gym_minutes,trained,weight_kg
```

There is one row per meal item. A meal with no items has one row with its meal columns filled and the item columns (`item_index` through `fat_g`) blank. A day with activity but no meals has one day-only row: `date`, `steps`, `gym_minutes`, `trained` and `weight_kg` filled, every meal column blank. A day with neither meals nor activity produces no row.

Day fields are repeated on every row for that date. This keeps a day's activity even if a consumer keeps only rows with items, and makes each row self-contained.

| Column | Value |
| --- | --- |
| `date` | the user's local date, `YYYY-MM-DD` |
| `meal_id` | the meal's Cupful UUID, or blank on a day-only row |
| `item_index` | 0-based item position within the meal; blank when the meal has no items |
| `meal` | `breakfast`, `lunch`, `dinner` or `snack` |
| `description` | the meal description as saved |
| `food` | the item name |
| `portion` | the item's portion text, e.g. `1 cup` or `120 g` |
| `kcal`, `protein_g`, `carbs_g`, `fat_g` | that item's values; macros are grams |
| `steps`, `gym_minutes` | the day's values |
| `trained` | body parts joined with `;`, e.g. `legs;back`; blank when none |
| `weight_kg` | the day's weigh-in in kilograms |

Rows are grouped by date; within a date each meal's rows are consecutive and items follow their `item_index`.

### Numbers and empty cells

Numbers are written in their shortest round-trip form: `81.4`, not `81.40000000000001`; an integer has no decimal point. A zero is a value and is written as `0`. A blank cell means the value is absent or unknown, never zero. `steps=0` and blank steps are different days; the same goes for `0` kcal or `0 g` of a macro.

`weight_kg` is always kilograms in the file, even when the profile shows pounds. The profile's unit setting affects display only; it never changes what export writes or import stores.

### Formula guard

A spreadsheet treats a cell beginning with `=`, `+`, `-` or `@` as a formula. Export prefixes such a value with a single `'` (so `-5 a day` becomes `'-5 a day`), and import removes one leading `'` from text fields before storing the value. This makes export → import a faithful round trip while keeping the file safe to open in Excel, Numbers or Google Sheets. No other escaping is applied.

## Import

### Grouping and ids

- Rows with the same `meal_id` form one meal. That id's value must not change within the group: if rows that share a `meal_id` disagree on date, meal type or description, the import is rejected with the line number. A food log row is either part of one meal or it is not.
- **An existing meal of yours with that UUID is updated.** This is how Cupful's own export round-trips, and how an edited Fuel Ledger file updates rows that originally came from Cupful.
- **Any other `meal_id` becomes `UUIDv5(namespace, "<your user id>:<meal_id>")`.** The namespace is a fixed Cupful UUID. Using the user id in the name means two accounts importing the same file do not produce clashing meal ids. The same input file on the same account always produces the same ids, so re-import is idempotent.
- Each item becomes `UUIDv5(<meal UUID>, "<item_index>")`. `item_index` is 0-based and is the item's position in the file.

Import derives item ids only for new meals. When an existing meal is updated from a Cupful export, its existing items keep their ids by position (see "Re-import and edits").

### Files without `meal_id` (older Fuel Ledger exports)

If `meal_id` is blank, import falls back to grouping consecutive rows that share `date`, `meal` and `description`. The meal id is `UUIDv5(namespace, "<user id>:<hash of the group's rows>:<occurrence>")`, where the occurrence counter distinguishes identical groups later in the same file. Two identical meals next to each other therefore both import.

This fallback cannot tell one meal with two identical items from two identical meals: in both cases the rows are indistinguishable. Files without `meal_id` are best treated as a one-time import. Editing a description in the source and re-importing creates a new meal, because the description is part of the grouping key. Keep `meal_id` if you want edits to update existing meals.

### Re-import and edits

Importing the same file twice changes nothing. Meal and item ids are deterministic, values that already match are skipped, and no row's timestamps move, so an unrelated newer edit is not overwritten by an old file.

With `meal_id`, an edited description, meal type, item name, portion or nutrient value updates that meal. Without `meal_id`, the edited row groups differently and becomes a new meal.

When the file is Cupful's own export (or another file carrying the same UUIDs), the import merges with the existing meal rather than replacing everything: the CSV columns are applied, and the values the CSV cannot carry are kept:

- item grams, food links (`food_id`) and item UUIDs by position;
- the meal's `scale` ("×1.5" display), `source` (`ai`, `manual`, `repeat` or `import`), assumptions, confidence and provider link;
- for days, the source and timestamp of fields whose value did not change.

A newly created meal gets `source: 'import'`, scale 1 and no food links. Its items have no grams, because the file carries only portion text and nutrients.

Because UUIDv5 ids are not time-ordered (unlike the UUIDv7s Cupful normally creates), imported meals within one date have no guaranteed chronological order. The day total is unaffected; only the ordering of meals in a list may look arbitrary.

### Activity fields

Steps, gym minutes, body parts and weight on a row apply to that row's date. All rows for a date carry the same values; if they differ, the last row for that date wins. Imported steps and gym minutes have source `import`.

Precedence follows `POST /api/sync`:

- **Manual survives.** An imported step count or gym session never overwrites a value that was entered by hand. Among automated values (`import`, `shortcut`, `google`), the newest write wins.
- **Weight and body parts fill only.** `weight_kg` and `trained` have no source column. Import sets them only when the day has no value (`NULL`, or `[]` for body parts); it never overwrites or clears an existing weight or list. A second import therefore cannot undo a later weigh-in.
- Import does not create rows for blank activity cells and does not change a day's fields to blank.

### Limits and scope

- At most 50,000 data rows per request; the body is capped at 10 MB. Export has no date-range filter, so a very long history may exceed the row cap. Split the file, or use a JSON backup for the whole account.
- The importer accepts only the header above, valid `YYYY-MM-DD` dates and the four meal types. Dates after tomorrow in the profile's timezone are rejected, as in sync.
- Import only touches the signed-in user's rows.
