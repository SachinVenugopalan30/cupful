// SPDX-License-Identifier: AGPL-3.0-or-later
// Shared logout flow: count queued writes, then end the server session before clearing this
// device's copy of the account. The caller shows the confirmation and any error.

import { auth } from './api';
import { connection } from './offline/state.svelte';
import { clearUser, countFor } from './offline/queue';
import { clearPreviousUser, clearSessionCache } from './offline/session-cache';

/** Queued writes for the signed-in account. */
export async function pendingChanges(): Promise<number> {
	const userId = connection.activeUserId;
	return userId === null ? 0 : countFor(userId);
}

/** End the session and clear the local copy, then send the browser to the login screen. */
export async function signOut(): Promise<void> {
	const userId = connection.activeUserId;
	// Server session first: if this fails, nothing local has been deleted.
	await auth.logout();
	if (userId !== null) {
		try {
			await clearUser(userId);
			await clearSessionCache(userId);
			await clearPreviousUser();
		} catch {
			// The next login clears any leftovers; being logged out still matters more.
		}
	}
	// Full reload: no state from this account survives into the next one.
	location.assign('/login');
}

/** The warning shown before discarding queued writes. */
export function logoutWarning(waiting: number): string {
	return `${waiting} ${waiting === 1 ? 'change hasn’t' : 'changes haven’t'} synced. Logging out discards ${
		waiting === 1 ? 'it' : 'them'
	} from this device.`;
}
