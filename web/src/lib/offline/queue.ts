// SPDX-License-Identifier: AGPL-3.0-or-later
// STUB. The write queue lands in its own change; these signatures are the contract screens and
// the sync path already code against. Do not post to /api/sync from here yet.
// ponytail: no queue behavior, replaced by the queue feature agent.

import type { Op } from '../sync/index.ts';
import { connection } from './state.svelte.ts';

export async function start(userId: number): Promise<void> {
	connection.activeUserId = userId;
}

export function stop(): void {}

export async function enqueue(_ops: Op[], _userId: number): Promise<void> {}

export async function flush(_userId?: number): Promise<void> {}

export async function syncNow(): Promise<void> {}

export async function retry(_opId: string): Promise<void> {}

export async function discard(_opId: string): Promise<void> {}

export async function clearUser(_userId: number): Promise<void> {}

export async function countFor(_userId: number): Promise<number> {
	return 0;
}
