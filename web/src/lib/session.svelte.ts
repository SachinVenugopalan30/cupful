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
import { clearSessionCache, loadSessionCache, saveSession } from './offline/session-cache.ts';
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
		session.auth = cached.auth;
		session.user = cached.auth.user;
		session.profile = cached.profile;
		// Offline, so start() has nothing to flush.
		await start(cached.userId);
		if (!day.date) day.date = today();
		return;
	}

	if (s.user) {
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

	// Logged out: drop the previous user's cached reads, then the active marker. The layout redirects.
	const previous = connection.activeUserId;
	if (previous !== null) await clearUserCache(previous);
	await clearSessionCache();
	connection.activeUserId = null;
	session.auth = s;
	session.user = null;
	session.profile = null;
}

const PUBLIC = ['/login', '/setup'];

/** Where `path` must go for this auth state, or null to stay. */
export function redirectFor(path: string, s: AuthState): string | null {
	if (s.needs_setup) return path === '/setup' ? null : '/setup';
	if (!s.user) return path === '/login' ? null : '/login';
	return PUBLIC.includes(path) ? '/' : null;
}
