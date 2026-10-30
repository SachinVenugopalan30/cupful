// SPDX-License-Identifier: AGPL-3.0-or-later
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AuthState, Profile } from './api';
import { ApiError } from './api';
import { getCachedBody, putCachedBody } from './offline/cache.ts';
import { clearUserData } from './offline/db.ts';
import {
	clearPreviousUser,
	clearSessionCache,
	loadPreviousUser,
	loadSessionCache,
	saveSession
} from './offline/session-cache.ts';
import { connection } from './offline/state.svelte.ts';
import { day, loadSession, redirectFor, session, today } from './session.svelte';

const mocks = vi.hoisted(() => ({
	authState: vi.fn(),
	profileGet: vi.fn(),
	start: vi.fn()
}));

vi.mock('./api', async (importOriginal) => {
	const actual = await importOriginal<typeof import('./api')>();
	return {
		...actual,
		auth: { state: mocks.authState },
		profile: { get: mocks.profileGet }
	};
});

vi.mock('./offline/queue.ts', () => ({ start: mocks.start }));

const user = { id: 1, username: 'a', is_admin: true };
const ada: AuthState = {
	needs_setup: false,
	user: { id: 7101, username: 'ada', is_admin: false },
	identity_mode: null
};
const bob: AuthState = {
	needs_setup: false,
	user: { id: 7102, username: 'bob', is_admin: false },
	identity_mode: null
};
const profile: Profile = {
	timezone: 'Europe/London',
	weight_unit: 'kg',
	height_unit: 'cm',
	version: null
};

describe('redirectFor', () => {
	it('sends everything to setup on first run', () => {
		const firstRun: AuthState = { needs_setup: true, user: null, identity_mode: null };
		expect(redirectFor('/', firstRun)).toBe('/setup');
		expect(redirectFor('/login', firstRun)).toBe('/setup');
		expect(redirectFor('/setup', firstRun)).toBeNull();
	});

	it('sends logged-out visitors to login, setup included once it is done', () => {
		const loggedOut: AuthState = { needs_setup: false, user: null, identity_mode: null };
		expect(redirectFor('/', loggedOut)).toBe('/login');
		expect(redirectFor('/settings', loggedOut)).toBe('/login');
		expect(redirectFor('/setup', loggedOut)).toBe('/login');
		expect(redirectFor('/login', loggedOut)).toBeNull();
	});

	it('keeps an invite open to a logged-out visitor', () => {
		expect(redirectFor('/invite', { needs_setup: false, user: null, identity_mode: null })).toBeNull();
	});

	it('sends an invite to setup before the first admin exists', () => {
		expect(redirectFor('/invite', { needs_setup: true, user: null, identity_mode: null })).toBe(
			'/setup'
		);
	});

	it('sends logged-in users away from login, setup and invite only', () => {
		const signedIn: AuthState = { needs_setup: false, user, identity_mode: null };
		expect(redirectFor('/login', signedIn)).toBe('/');
		expect(redirectFor('/setup', signedIn)).toBe('/');
		expect(redirectFor('/invite', signedIn)).toBe('/');
		expect(redirectFor('/settings', signedIn)).toBeNull();
		expect(redirectFor('/', signedIn)).toBeNull();
	});
});

describe('loadSession', () => {
	beforeEach(async () => {
		for (const fn of [mocks.authState, mocks.profileGet, mocks.start]) fn.mockReset();
		session.auth = null;
		session.user = null;
		session.profile = null;
		day.date = '';
		connection.activeUserId = null;
		connection.online = true;
		for (const id of [7101, 7102, 7103]) {
			await clearSessionCache(id);
			await clearUserData(id);
		}
		await clearSessionCache();
		await clearPreviousUser();
	});

	afterEach(() => vi.unstubAllGlobals());

	it('starts the queue, saves the session and loads the profile when online', async () => {
		vi.stubGlobal('navigator', { onLine: true });
		mocks.authState.mockResolvedValue(ada);
		mocks.profileGet.mockResolvedValue(profile);

		await loadSession();

		expect(mocks.start).toHaveBeenCalledWith(7101);
		expect(session.auth).toEqual(ada);
		expect(session.user).toEqual(ada.user);
		expect(session.profile).toEqual(profile);
		expect(connection.activeUserId).toBe(7101);
		expect(connection.online).toBe(true);
		expect(day.date).not.toBe('');
		expect(await loadSessionCache()).toEqual({ userId: 7101, auth: ada, profile });
	});

	it('boots from the cached session when the network fails', async () => {
		vi.stubGlobal('navigator', { onLine: true });
		await saveSession(7102, bob, profile);
		mocks.authState.mockRejectedValue(new TypeError('Failed to fetch'));

		await loadSession();

		expect(mocks.profileGet).not.toHaveBeenCalled();
		expect(mocks.start).toHaveBeenCalledWith(7102);
		expect(session.auth).toEqual(bob);
		expect(session.user).toEqual(bob.user);
		expect(session.profile).toEqual(profile);
		expect(connection.activeUserId).toBe(7102);
		expect(connection.online).toBe(false);
		expect(day.date).toBe(today());
	});

	it('clears the previous cached user when another account signs in', async () => {
		await saveSession(7102, bob, profile);
		await putCachedBody(7102, '/api/meals?date=2026-10-29', { meals: [] });
		mocks.authState.mockResolvedValue(ada);
		mocks.profileGet.mockResolvedValue(profile);

		await loadSession();

		expect(await getCachedBody(7102, '/api/meals?date=2026-10-29')).toBeUndefined();
		expect(await loadSessionCache()).toEqual({ userId: 7101, auth: ada, profile });
		expect(session.user).toEqual(ada.user);
	});

	it('rethrows the network failure when nothing is cached', async () => {
		mocks.authState.mockRejectedValue(new TypeError('Failed to fetch'));

		await expect(loadSession()).rejects.toThrow('Failed to fetch');
		expect(session.auth).toBeNull();
		expect(session.user).toBeNull();
		expect(connection.activeUserId).toBeNull();
	});

	it('clears the previous user’s cached reads when the server says logged out', async () => {
		connection.activeUserId = 7103;
		await saveSession(
			7103,
			{ needs_setup: false, user: { id: 7103, username: 'c', is_admin: false }, identity_mode: null },
			profile
		);
		await putCachedBody(7103, '/api/meals?date=2026-10-29', { meals: [] });
		mocks.authState.mockResolvedValue({ needs_setup: false, user: null, identity_mode: null });

		await loadSession();

		expect(connection.activeUserId).toBeNull();
		expect(session.auth).toEqual({ needs_setup: false, user: null, identity_mode: null });
		expect(session.user).toBeNull();
		expect(session.profile).toBeNull();
		expect(mocks.start).not.toHaveBeenCalled();
		expect(await getCachedBody(7103, '/api/meals?date=2026-10-29')).toBeUndefined();
		expect(await loadSessionCache()).toBeNull();
		expect(await loadPreviousUser()).toEqual({ userId: 7103, username: 'c' });
	});

	it('rethrows an HTTP error rather than treating it as offline', async () => {
		vi.stubGlobal('navigator', { onLine: true });
		await saveSession(7102, bob, profile);
		mocks.authState.mockRejectedValue(new ApiError(500, 'boom'));

		await expect(loadSession()).rejects.toBeInstanceOf(ApiError);
		expect(connection.online).toBe(true);
		expect(await loadSessionCache()).not.toBeNull();
	});
});
