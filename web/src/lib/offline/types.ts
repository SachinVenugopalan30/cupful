// SPDX-License-Identifier: AGPL-3.0-or-later
// Shared types for the offline layer. Nothing here runs; it is the contract between the
// IndexedDB stores, the write queue and the screens.

import type { Op } from '../sync/index.ts';

/** Lifecycle of a queued op. `pending` and `sending` are still reflected in the UI overlay. */
export type QueueStatus = 'pending' | 'sending' | 'stale' | 'rejected' | 'failed';

/** One queued write, stored in IndexedDB `queue` and keyed by user for account isolation. */
export type QueuedOp = {
	op_id: string;
	user_id: number;
	op: Op;
	status: QueueStatus;
	error?: string;
	/** Unix milliseconds when it was queued. */
	queued_at: number;
	attempts: number;
};

/** Why the queue is not flushing. `auth` means re-login is needed; `identity` means the queue
 * belongs to a different user than the active session. */
export type Pause = null | 'offline' | 'auth' | 'identity';
