// SPDX-License-Identifier: AGPL-3.0-or-later
// Shared app state: who is logged in, their profile, and the selected day.
// Mutate the properties; never reassign the exported objects.

import { auth, profile as profileApi, type AuthState, type AuthUser, type Profile } from './api';
import { todayIn } from './date';

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

/** Loads auth state, and the profile when logged in. Call again after login, setup or profile save. */
export async function loadSession(): Promise<void> {
	const s = await auth.state();
	session.profile = s.user ? await profileApi.get() : null;
	session.user = s.user;
	session.auth = s;
	if (s.user && !day.date) day.date = today();
}

const PUBLIC = ['/login', '/setup'];

/** Where `path` must go for this auth state, or null to stay. */
export function redirectFor(path: string, s: AuthState): string | null {
	if (s.needs_setup) return path === '/setup' ? null : '/setup';
	if (!s.user) return path === '/login' ? null : '/login';
	return PUBLIC.includes(path) ? '/' : null;
}
