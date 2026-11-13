// SPDX-License-Identifier: AGPL-3.0-or-later
// One-line summaries for Settings rows whose state is already loaded: no extra fetch to show them.

import type { GoogleStatus } from '#lib/api.ts';

/** The sync row: the queue's size, from state the shell already holds. */
export function syncSummary(waiting: number): string {
	if (waiting === 0) return 'All changes synced.';
	return `${waiting} ${waiting === 1 ? 'change' : 'changes'} waiting.`;
}

/** The Google row; undefined keeps the row's static description (status not loaded, or unconfigured). */
export function googleSummary(
	status: GoogleStatus | null,
	time: (iso: string) => string = (iso) =>
		new Date(iso).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })
): string | undefined {
	if (!status?.configured) return undefined;
	if (!status.connected) return 'Not connected';
	if (status.needs_reconnect) return 'Needs reconnecting';
	return status.last_sync_at
		? `Connected · synced ${time(status.last_sync_at)}`
		: 'Connected · not synced yet';
}
