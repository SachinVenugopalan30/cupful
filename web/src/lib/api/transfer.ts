// SPDX-License-Identifier: AGPL-3.0-or-later
// Import, backup and restore: CSV upload, JSON restore, and attachment downloads.

import { api, request, send } from './http.ts';
import type { ImportSummary, RestoreSummary } from './types.ts';
import { fallbackFilename, responseFilename } from '../settings/data.ts';

/** Import a Fuel Ledger CSV. Re-importing the same file changes nothing. */
export const importCsv = (text: string) =>
	request<ImportSummary>('POST', '/api/import/fuel-ledger', text, 'text/csv');

/** Replace this account's logged data with a parsed backup document. */
export const restoreBackup = (backup: unknown) =>
	api.post<RestoreSummary>('/api/restore', { confirm: true, backup });

/**
 * GET an attachment, taking its filename from Content-Disposition and falling back to a local-date
 * name. `Settings → Data` turns the blob into a temporary object URL to download it.
 */
export async function download(path: string): Promise<{ blob: Blob; filename: string }> {
	const res = await send('GET', path);
	const localDate = new Date().toLocaleDateString('en-CA');
	return {
		blob: await res.blob(),
		filename:
			responseFilename(res.headers.get('Content-Disposition')) ?? fallbackFilename(path, localDate)
	};
}
