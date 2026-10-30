// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from 'vitest';
import { ApiError } from './api/errors.ts';
import { inviteErrorMessage, inviteToken, inviteUrl } from './invites.ts';

describe('inviteUrl', () => {
	it('builds an absolute link with an encoded token', () => {
		expect(inviteUrl('https://cup.example', 'a+b/c=')).toBe(
			'https://cup.example/invite?token=a%2Bb%2Fc%3D'
		);
	});

	it('does not double the slash when the origin ends with one', () => {
		expect(inviteUrl('https://cup.example/', 'abc')).toBe('https://cup.example/invite?token=abc');
	});
});

describe('inviteToken', () => {
	it('reads the token from a query string', () => {
		expect(inviteToken('?token=abc')).toBe('abc');
		expect(inviteToken('token=abc&other=1')).toBe('abc');
	});

	it('returns an empty string when the token is missing', () => {
		expect(inviteToken('')).toBe('');
		expect(inviteToken('?other=1')).toBe('');
	});
});

describe('inviteErrorMessage', () => {
	it('explains a used, expired or unknown invite', () => {
		expect(inviteErrorMessage(new ApiError(410, 'gone'))).toMatch(/used, has expired or is not valid/);
	});

	it('explains a taken username or identity mode', () => {
		expect(inviteErrorMessage(new ApiError(409, 'conflict'))).toMatch(/username is taken/);
	});

	it('explains the invite rate limit', () => {
		expect(inviteErrorMessage(new ApiError(429, 'slow down'))).toMatch(/Wait an hour/);
	});

	it('keeps the server message for other errors', () => {
		expect(inviteErrorMessage(new ApiError(500, 'boom'))).toBe('boom');
	});

	it('names a network failure', () => {
		expect(inviteErrorMessage(new TypeError('Failed to fetch'))).toMatch(/Can't reach Cupful/);
	});
});
