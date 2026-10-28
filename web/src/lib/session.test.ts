// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import { redirectFor } from './session.svelte';

const user = { id: 1, username: 'a', is_admin: true };

describe('redirectFor', () => {
	it('sends everything to setup on first run', () => {
		expect(redirectFor('/', { needs_setup: true, user: null })).toBe('/setup');
		expect(redirectFor('/login', { needs_setup: true, user: null })).toBe('/setup');
		expect(redirectFor('/setup', { needs_setup: true, user: null })).toBeNull();
	});

	it('sends logged-out visitors to login, setup included once it is done', () => {
		expect(redirectFor('/', { needs_setup: false, user: null })).toBe('/login');
		expect(redirectFor('/settings', { needs_setup: false, user: null })).toBe('/login');
		expect(redirectFor('/setup', { needs_setup: false, user: null })).toBe('/login');
		expect(redirectFor('/login', { needs_setup: false, user: null })).toBeNull();
	});

	it('sends logged-in users away from login and setup only', () => {
		expect(redirectFor('/login', { needs_setup: false, user })).toBe('/');
		expect(redirectFor('/setup', { needs_setup: false, user })).toBe('/');
		expect(redirectFor('/settings', { needs_setup: false, user })).toBeNull();
		expect(redirectFor('/', { needs_setup: false, user })).toBeNull();
	});
});
