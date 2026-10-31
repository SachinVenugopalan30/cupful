// SPDX-License-Identifier: AGPL-3.0-or-later
// Same-origin JSON client for the Cupful API. This barrel keeps one import path for screens;
// the implementation is grouped by resource under `./api/`.
//
// Logged data (meals, days) is written only through `sendOps` in `#lib/sync`; everything here is
// reads and online-only config REST.

export type * from './api/types.ts';
export { ApiError, errorMessage, parseError } from './api/errors.ts';
export { api, cachedGet } from './api/http.ts';
export { getBudget, getDay, getDays, getMeals, getWorkouts, getTrends, health } from './api/reads.ts';
export { auth, profile } from './api/account.ts';
export { invites } from './api/invites.ts';
export { estimate } from './api/nutrition.ts';
export { foods, products } from './api/products.ts';
export { providers } from './api/providers.ts';
export { ingestTokens } from './api/ingest.ts';
export { google } from './api/google.ts';
export { download, importCsv, restoreBackup } from './api/transfer.ts';
