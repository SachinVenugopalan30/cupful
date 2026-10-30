# Backup and restore

Cupful has two kinds of copy:

- **JSON backup** (`GET /api/backup`, `POST /api/restore`) is a portable snapshot of one account's data. It moves an account between users or servers and can be restored on its own.
- **A copy of the SQLite file** is a snapshot of the whole instance, including every account and the server's configuration and secrets. This is what a nightly backup should be.

Neither is a replacement for the other. A JSON backup restores data into a working Cupful; a database copy restores the server itself.

## What the JSON backup contains

`GET /api/backup` returns one JSON document for the signed-in user:

- the profile (timezone and unit preferences) and every profile version;
- every day: steps and gym minutes with their sources, body parts trained, weight, and the per-field timestamps;
- every live meal and its items: dates, meal types, descriptions, scale, assumptions, confidence, source, provider link, item names, portions, grams, nutrients and food links;
- the foods those meals and the user's aliases, portions and saved products refer to, with their `food_portions` rows;
- the user's own aliases;
- the user's remembered portions (`user_portions`);
- the user's saved products (`user_products`, including last servings and last-used time).

A backup deliberately excludes:

- AI provider keys and provider settings (instance-wide, encrypted);
- ingest tokens and the Google Health connection and tokens;
- `sync_ops` bookkeeping and sessions;
- deleted meals (tombstones);
- the account's username and password hash.

It is a data restore, not a server clone. Restoring a backup never brings back an old password, provider key or phone token, and it never changes another user's data.

## Restoring a JSON backup

```sh
curl -b cookies.txt https://cupful.example.ts.net/api/backup -o cupful-backup.json

{ printf '{"confirm":true,"backup":'; cat cupful-backup.json; printf '}'; } |
  curl -b cookies.txt -X POST https://cupful.example.ts.net/api/restore \
    -H 'X-Cupful: 1' \
    -H 'Content-Type: application/json' \
    --data-binary @-
```

The request body is JSON, capped at 32 MB, and must contain `confirm: true`; without it the restore is refused. The endpoint requires the CSRF header like every other mutating request.

A restore is a **full replacement of this account's logs and private foods**, not a merge:

- The account's meals (including tombstones), day rows, profile versions and private foods are deleted and replaced by the snapshot. Meals and days that are not in the backup are gone.
- The profile's timezone and unit preferences are replaced too. "Today" and the dates in the restored data are then interpreted in the backup's timezone.
- The user's own aliases and remembered portions are replaced, as are the saved-product list and its last-used values.
- **Shared foods are read-only.** A restore never modifies a staple, USDA, Open Food Facts or shared label row, and never changes another user's food. If a food referenced by the backup is missing on this instance, restore creates a private copy owned by the user, preserving its name and nutrients. Meal items, aliases, portions and products point at that copy.
- Remembered sync op ids for the user are cleared, so pre-restore write bookkeeping does not interfere with the restored state.
- The whole restore runs in one transaction. If any part fails (malformed document, invalid value, a missing referenced row), it rolls back and the account is exactly as it was before.

Restoring the same backup twice gives the same result: the second restore replaces the first with identical data. Restoring an older backup is also allowed and is how you go back in time; it discards everything logged since that snapshot.

## Nightly SQLite copy

A JSON backup is per account. For the whole instance, copy the database with SQLite's own online backup command rather than `cp`. The Cupful image is distroless (no shell, no `sqlite3`) and the data lives in a Docker named volume, not a host folder, so run a throwaway container that mounts the same volume.

Find the volume's real name first. Compose prefixes it with the project name, which is the directory name (`cupful`) unless you set `name:` or `COMPOSE_PROJECT_NAME`:

```sh
docker volume ls | grep cupful-data      # e.g. cupful_cupful-data
```

Then, from the folder where backups should land (this works on Linux and Docker Desktop for macOS alike):

```sh
mkdir -p backups
docker run --rm -v cupful_cupful-data:/data -v "$PWD/backups:/backups" alpine:3.20 sh -c \
  'apk add -q sqlite && sqlite3 /data/cupful.db "VACUUM INTO '\''/backups/cupful-$(date +%F).db'\''"'
```

`VACUUM INTO` writes a consistent snapshot while Cupful is running, including everything in the WAL, and refuses to overwrite an existing file (a second run on the same day fails harmlessly). Do not mount the data volume read-only: SQLite needs to open the WAL files. Keep `backups/` outside that volume, and ideally on another disk or machine, so a lost volume does not lose the backups. The `'\''` is a literal single quote inside the single-quoted script; `$(date +%F)` is expanded by the container's shell.

A nightly cron line (`crontab -e`; use an absolute path, and `%` must be escaped as `\%` in crontab):

```cron
15 3 * * * cd /srv/cupful-backups && docker run --rm -v cupful_cupful-data:/data -v "$PWD/backups:/backups" alpine:3.20 sh -c 'apk add -q sqlite && sqlite3 /data/cupful.db "VACUUM INTO '\''/backups/cupful-$(date +\%F).db'\''"'
```

Old snapshots are not pruned; add `find backups -name 'cupful-*.db' -mtime +30 -delete` to the job if you want a 30-day window.

To copy `secret.key` (see below) the same way:

```sh
docker run --rm -v cupful_cupful-data:/data -v "$PWD/backups:/backups" alpine:3.20 \
  cp /data/secret.key /backups/secret.key
```

### Restoring a snapshot

Stop Cupful, put the snapshot in the volume, and start it again. The old WAL files must go, or SQLite may replay them over the restored file. The image runs as uid 65532, so the restored files need that owner:

```sh
docker compose stop cupful
docker run --rm -v cupful_cupful-data:/data -v "$PWD/backups:/backups" alpine:3.20 sh -c \
  'rm -f /data/cupful.db-wal /data/cupful.db-shm &&
   cp /backups/cupful-2026-10-30.db /data/cupful.db &&
   chown 65532:65532 /data/cupful.db'
docker compose start cupful
```

If you also saved `secret.key` and the volume lost it, copy it back the same way (`cp /backups/secret.key /data/secret.key && chown 65532:65532 /data/secret.key`) before starting.

If the key was generated by Cupful (that is, `CUPFUL_SECRET_KEY` is not set), also copy `secret.key` with the database (command above). If the key comes from the environment, back up that value instead. Restrict access to both: the database contains password hashes and encrypted provider secrets, and the key decrypts the latter.

Losing the key does not lose your food log. Only two things are encrypted with it: AI provider keys and Google Health tokens. Cupful stores a check value and refuses to start when the key does not match. Set the old key again, or, if it is truly lost, clear the encrypted provider and Google rows from the database and enter those credentials again. Accounts, meals, days, foods and passwords are all readable without the key. That is also why the nightly copy should include `secret.key` when it exists: it saves re-entering those keys, nothing more.

## CSV is not a backup

The CSV export in [`import-export.md`](import-export.md) contains the food log and daily activity, and it round-trips into the same account well. It does not contain profile versions, grams, food links, aliases, remembered portions, saved products, meal scale, or sources beyond what a column can say, and its meal UUIDs belong to one instance. Use CSV to move a log to or from Fuel Ledger, and a JSON backup or a database copy to preserve Cupful.
