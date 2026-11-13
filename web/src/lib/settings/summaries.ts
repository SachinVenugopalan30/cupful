// SPDX-License-Identifier: AGPL-3.0-or-later
// One-line summaries for Settings rows whose state is already loaded: no extra fetch to show them.

import type { GoogleStatus } from '#lib/api.ts';

/** The sync row, from the queue the shell already holds. Changes that need a decision come first. */
export function syncSummary(ops: readonly { status: string }[]): string {
	const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
	const attention = ops.filter((o) => o.status === 'stale' || o.status === 'rejected' || o.status === 'failed');
	if (attention.length) return `${plural(attention.length, 'change needs', 'changes need')} attention.`;
	if (ops.length) return `${plural(ops.length, 'change', 'changes')} waiting.`;
	return 'All changes synced.';
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
