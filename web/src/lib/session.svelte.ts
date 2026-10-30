// SPDX-License-Identifier: AGPL-3.0-or-later
// Shared app state: who is logged in, their profile, and the selected day.
// Mutate the properties; never reassign the exported objects.

import {
	ApiError,
	auth,
	profile as profileApi,
	type AuthState,
	type AuthUser,
	type Profile
} from './api';
import { todayIn } from './date';
import { clearUserCache } from './offline/cache.ts';
import { start } from './offline/queue.ts';
import {
	clearSessionCache,
	loadSessionCache,
	savePreviousUser,
	saveSession
} from './offline/session-cache.ts';
import { connection } from './offline/state.svelte.ts';

export const session = $state({
	/** Set once GET /api/auth/state has answered. */
	auth: null as AuthState | null,
	user: null as AuthUser | null,
	profile: null as Profile | null
});

/** The day Today shows, `YYYY-MM-DD`. Set to the user's today when the profile loads. */
export const day = $state({ date: '' });

/** The user's today in their profile timezone (the browser's until the profile is loaded). */
export function today(): string {
	return todayIn(session.profile?.timezone ?? Intl.DateTimeFormat().resolvedOptions().timeZone);
}

/**
 * Loads auth state, and the profile when logged in. Call again after login, setup or profile save.
 * Offline, it boots from the last user's cached session instead, so their data stays readable.
 */
export async function loadSession(): Promise<void> {
	if (typeof navigator !== 'undefined') connection.online = navigator.onLine;

	let s: AuthState;
	try {
		s = await auth.state();
	} catch (err) {
		// Only a network failure falls back; an HTTP error is the server's answer.
		if (err instanceof ApiError) throw err;
		const cached = await loadSessionCache();
		// No cached session: the layout shows its "Can't reach Cupful" screen.
		if (!cached) throw err;
		connection.activeUserId = cached.userId;
		connection.online = false;
		// Set the day before session.user: the layout renders DayNav as soon as user is non-null,
		// and an empty date would format as an invalid Date.
		session.profile = cached.profile;
		if (!day.date) day.date = today();
		// Offline, so start() has nothing to flush.
		await start(cached.userId);
		session.auth = cached.auth;
		session.user = cached.auth.user;
		return;
	}

	if (s.user) {
		// A different account signed in (a new login, or another tab). Drop the previous user's
		// cached reads and session data. Their queued writes are left for the explicit switch flow,
		// which warns before discarding them.
		const cached = await loadSessionCache().catch(() => null);
		if (cached && cached.userId !== s.user.id) {
			await clearUserCache(cached.userId);
			await clearSessionCache(cached.userId);
		}
		connection.activeUserId = s.user.id;
		const profile = await profileApi.get();
		await saveSession(s.user.id, s, profile);
		await start(s.user.id);
		session.auth = s;
		session.user = s.user;
		session.profile = profile;
		if (!day.date) day.date = today();
		return;
	}

	// Logged out: drop the cached user's reads and session. The queue is kept so unsynced changes
	// can still sync after signing in again. The layout redirects.
	const cached = await loadSessionCache().catch(() => null);
	if (cached) {
		await savePreviousUser(cached.userId, cached.auth.user?.username ?? 'another account');
		await clearUserCache(cached.userId);
		await clearSessionCache(cached.userId);
	} else {
		await clearSessionCache();
	}
	connection.activeUserId = null;
	session.auth = s;
	session.user = null;
	session.profile = null;
}

/** Routes a signed-in user is sent away from. */
const PUBLIC = ['/login', '/setup', '/invite'];
/** Routes a logged-out visitor may open. `/invite` carries a one-time token. */
const LOGGED_OUT = ['/login', '/invite'];

/** Where `path` must go for this auth state, or null to stay. */
export function redirectFor(path: string, s: AuthState): string | null {
	if (s.needs_setup) return path === '/setup' ? null : '/setup';
	if (!s.user) return LOGGED_OUT.includes(path) ? null : '/login';
	return PUBLIC.includes(path) ? '/' : null;
}
