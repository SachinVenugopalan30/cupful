// SPDX-License-Identifier: AGPL-3.0-or-later
import { beforeEach, describe, expect, it } from 'vitest';
import type { AuthState, Health, Profile } from '../api.ts';
import {
	clearSessionCache,
	loadHealth,
	loadSessionCache,
	saveHealth,
	saveSession
} from './session-cache.ts';

const auth: AuthState = {
	needs_setup: false,
	user: { id: 1, username: 'ada', is_admin: false }
};

const profile: Profile = {
	timezone: 'Europe/London',
	weight_unit: 'kg',
	height_unit: 'cm',
	version: null
};

const otherAuth: AuthState = {
	needs_setup: false,
	user: { id: 2, username: 'bob', is_admin: false }
};
const otherProfile: Profile = { ...profile, timezone: 'America/New_York' };

beforeEach(async () => {
	await clearSessionCache(1);
	await clearSessionCache(2);
	await clearSessionCache();
});

describe('session cache', () => {
	it('saves and loads the active user’s auth and profile', async () => {
		await saveSession(1, auth, profile);
		expect(await loadSessionCache()).toEqual({ userId: 1, auth, profile });
	});

	it('switches the active user without mixing accounts', async () => {
		await saveSession(1, auth, profile);
		await saveSession(2, otherAuth, otherProfile);
		expect(await loadSessionCache()).toEqual({
			userId: 2,
			auth: otherAuth,
			profile: otherProfile
		});
	});

	it('clears one user and the active marker only when it matches', async () => {
		await saveSession(1, auth, profile);
		await saveSession(2, otherAuth, otherProfile);

		await clearSessionCache(1);
		expect(await loadSessionCache()).toEqual({
			userId: 2,
			auth: otherAuth,
			profile: otherProfile
		});

		await clearSessionCache(2);
		expect(await loadSessionCache()).toBeNull();
	});

	it('clearing without an id drops only the marker', async () => {
		await saveSession(1, auth, profile);
		await clearSessionCache();
		expect(await loadSessionCache()).toBeNull();
	});

	it('refuses a cached auth state belonging to a different user id', async () => {
		await saveSession(2, auth, otherProfile);
		expect(await loadSessionCache()).toBeNull();
	});

	it('caches health for the active user', async () => {
		const health: Health = { status: 'ok', version: '1.2.3', source_url: 'https://example.test' };
		await saveSession(1, auth, profile);
		await saveHealth(health);
		expect(await loadHealth()).toEqual(health);

		await saveSession(2, otherAuth, otherProfile);
		expect(await loadHealth()).toBeNull();
	});
});
