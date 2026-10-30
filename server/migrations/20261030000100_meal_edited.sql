-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Set once a meal has been rewritten through sync (or came from a restore); a CSV import never
-- overwrites such a meal.
ALTER TABLE meals ADD COLUMN edited INTEGER NOT NULL DEFAULT 0;
