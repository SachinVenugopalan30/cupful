<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# Backup and restore

Cupful has two kinds of copy.

- A **JSON backup** (`GET /api/backup`, `POST /api/restore`) is a portable snapshot of one account. It moves an account between users or servers.
- A **copy of the SQLite file** is a snapshot of the whole instance, including every account, the configuration and the secrets. This is what a nightly backup should be.

Neither replaces the other. A JSON backup restores data into a working Cupful. A database copy restores the server itself.

## What the JSON backup contains

`GET /api/backup` returns one JSON document for the signed-in user:

- the profile (timezone and unit preferences) and every profile version;
- every day: steps and gym minutes with their sources, body parts trained, weight, and per-field timestamps;
- every live meal and its items: dates, meal types, descriptions, scale, assumptions, confidence, source, provider link, item names, portions, grams, nutrients and food links;
- the foods those meals and the user's aliases, portions and saved products refer to, with their `food_portions` rows;
- the user's own aliases;
- the user's remembered portions (`user_portions`);
- the user's saved products (`user_products`, including last servings and last-used time).

A backup deliberately excludes:

- AI provider keys and provider settings, which are instance-wide and encrypted;
- ingest tokens and the Google Health connection and tokens;
- `sync_ops` bookkeeping and sessions;
- deleted meals (tombstones);
- the account's username and password hash.

It is a data restore, not a server clone. Restoring a backup never brings back an old password, provider key or phone token. It never changes another user's data.

## Restoring a JSON backup

```sh
curl -b cookies.txt https://cupful.example.ts.net/api/backup -o cupful-backup.json

{ printf '{"confirm":true,"backup":'; cat cupful-backup.json; printf '}'; } |
  curl -b cookies.txt -X POST https://cupful.example.ts.net/api/restore \
    -H 'X-Cupful: 1' \
    -H 'Content-Type: application/json' \
    --data-binary @-
```

The request body is JSON, capped at 10 MB, and must contain `confirm: true`. Without it the restore is refused. The endpoint requires the CSRF header like every mutating request.

A restore fully replaces this account's logs and private foods. It does not merge.

- The account's live meals, day rows, profile versions and private foods are deleted and replaced by the snapshot. Meals and days that are not in the backup are gone. Deleted-meal tombstones are kept, so a phone replaying its offline queue after a restore cannot bring deleted meals back. If the backup holds a meal whose id is tombstoned here, the backup wins and the meal is revived with the backed-up content.
- The profile's timezone and unit preferences are replaced too. "Today" and the dates in the restored data are then interpreted in the backup's timezone.
- The user's aliases, remembered portions, saved products and their last-used values are replaced.
- Shared foods are read-only. A restore never modifies a staple, USDA, Open Food Facts or shared label row, and never changes another user's food. If a food referenced by the backup is missing on this instance, restore creates a private copy owned by the user, with the same name and nutrients. Meal items, aliases, portions and products point at that copy. Label-entered foods are private to their owner, so restore always recreates them as private foods, including older backups that carried them as shared rows.
- Remembered sync operation ids are kept, so operations a device already sent still answer `duplicate` after a restore.
- Two private foods in the backup with the same name collapse into one only if their barcode and nutrients also match.
- The whole restore runs in one transaction. If any part fails, it rolls back and the account is exactly as it was before.

Restoring the same backup twice gives the same result. Restoring an older backup is how you go back in time. It discards everything logged since that snapshot.

## Nightly SQLite copy

A JSON backup is per account. For the whole instance, copy the database with SQLite's online backup command instead of `cp`.

The Cupful image is distroless: no shell and no `sqlite3`. The data lives in a Docker named volume, not a host folder. Run a throwaway container that mounts the same volume.

Find the volume's real name. Compose prefixes it with the project name, which is the directory name (`cupful`) unless you set `name:` or `COMPOSE_PROJECT_NAME`:

```sh
docker volume ls | grep cupful-data
```

The examples below use `cupful_cupful-data`. Then, from the folder where backups should land:

```sh
mkdir -p backups
docker run --rm \
  -v cupful_cupful-data:/data \
  -v "$PWD/backups:/backups" \
  alpine:3.20 sh -c \
  'apk add -q sqlite && sqlite3 /data/cupful.db "VACUUM INTO '\''/backups/cupful-$(date +%F).db'\''"'
```

`VACUUM INTO` writes a consistent snapshot while Cupful is running, including everything in the WAL. It refuses to overwrite an existing file, so a second run on the same day fails harmlessly. Do not mount the data volume read-only; SQLite needs to open the WAL files. The `'\''` is a literal single quote inside the single-quoted script. `$(date +%F)` is expanded by the container's shell.

Keep `backups/` outside the data volume, and ideally on another disk or machine, so a lost volume does not lose the backups.

A nightly cron line (`crontab -e`). Use an absolute path. `%` must be escaped as `\%` in crontab.

```cron
15 3 * * * cd /srv/cupful-backups && docker run --rm -v cupful_cupful-data:/data -v "$PWD/backups:/backups" alpine:3.20 sh -c 'apk add -q sqlite && sqlite3 /data/cupful.db "VACUUM INTO '\''/backups/cupful-$(date +\%F).db'\''"'
```

Old snapshots are not pruned. Add `find backups -name 'cupful-*.db' -mtime +30 -delete` to the job for a 30-day window.

To copy `secret.key` the same way:

```sh
docker run --rm \
  -v cupful_cupful-data:/data \
  -v "$PWD/backups:/backups" \
  alpine:3.20 cp /data/secret.key /backups/secret.key
```

### Restore a snapshot

Stop Cupful, put the snapshot in the volume, and start it again. The old WAL files must go, or SQLite may replay them over the restored file. The image runs as uid 65532, so the restored files need that owner.

```sh
docker compose stop cupful
docker run --rm \
  -v cupful_cupful-data:/data \
  -v "$PWD/backups:/backups" \
  alpine:3.20 sh -c \
  'rm -f /data/cupful.db-wal /data/cupful.db-shm &&
   cp /backups/cupful-2026-10-30.db /data/cupful.db &&
   chown 65532:65532 /data/cupful.db'
docker compose start cupful
```

If you saved `secret.key` and the volume lost it, copy it back before starting:

```sh
docker run --rm -v cupful_cupful-data:/data -v "$PWD/backups:/backups" alpine:3.20 sh -c \
  'cp /backups/secret.key /data/secret.key && chown 65532:65532 /data/secret.key'
```

On a plain `docker run` install, replace `docker compose stop cupful` and `docker compose start cupful` with `docker stop cupful`, then run your usual `docker run` command again.

## The secret key

If Cupful generated the key (`CUPFUL_SECRET_KEY` is not set), copy `secret.key` with the database. If the key comes from the environment, back up that value instead. Restrict access to both. The database contains password hashes and encrypted provider secrets, and the key decrypts the latter.

Losing the key does not lose your food log. Only AI provider keys and Google Health tokens are encrypted with it. Cupful stores a check value and refuses to start when the key does not match.

Set the old key again. If it is truly lost, clear the encrypted rows and enter the credentials again. With `sqlite3` on a host that has the database:

```sh
sqlite3 cupful.db "UPDATE ai_providers SET api_key = NULL, extra_headers = NULL; DELETE FROM secret_check;"
```

Then start Cupful with a new key, or with none to generate one. Accounts, meals, days, foods and passwords are readable without the key.

## CSV is not a backup

The CSV export in [import-export.md](import-export.md) contains the food log and daily activity, and it round-trips into the same account well. It does not contain profile versions, grams, food links, aliases, remembered portions, saved products, meal scale, or sources beyond what a column can say. Its meal UUIDs belong to one instance.

Use CSV to move a log to or from Fuel Ledger. Use a JSON backup or a database copy to preserve Cupful.

From the app, backup, export, import and restore are under **Settings → Data**. They need a connection.
