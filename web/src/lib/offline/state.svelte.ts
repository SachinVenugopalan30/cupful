// SPDX-License-Identifier: AGPL-3.0-or-later
// Reactive offline state shared by the banner, queue detail and screens. Mutate these objects;
// never reassign them.

import type { Pause, QueuedOp } from './types.ts';

/** Connection, queue health and account identity, as the UI needs them. */
export const connection = $state({
	online: typeof navigator === 'undefined' ? true : navigator.onLine,
	paused: null as Pause,
	pending: 0,
	failed: 0,
	activeUserId: null as number | null
});

/** The full queued-op list for the detail view; the queue keeps it current. */
export const queueState = $state({ ops: [] as QueuedOp[] });

/** Bumped after the queue changes or a sync completes, so reads re-fetch and overlays refresh. */
export const dataVersion = $state({ n: 0 });

let listening = false;

/** Subscribes to browser online/offline events once. The layout calls this on mount. */
export function initConnection(): void {
	if (listening || typeof window === 'undefined') return;
	listening = true;
	connection.online = navigator.onLine;
	window.addEventListener('online', () => {
		connection.online = true;
	});
	window.addEventListener('offline', () => {
		connection.online = false;
	});
}
