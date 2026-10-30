// SPDX-License-Identifier: AGPL-3.0-or-later
// Invite links, token reading, and the plain-language errors the invite screen shows.

import { ApiError, errorMessage } from './api/errors.ts';

/** `/invite?token=…` for a token just created by an admin. */
export function inviteUrl(origin: string, token: string): string {
	return `${origin.replace(/\/+$/, '')}/invite?token=${encodeURIComponent(token)}`;
}

/** The token in a query string (with or without a leading `?`), or `''` when absent. */
export function inviteToken(search: string): string {
	return new URLSearchParams(search).get('token') ?? '';
}

/** A clear message for the invite screen. Used, conflict and rate-limit answers get fixed copy. */
export function inviteErrorMessage(e: unknown): string {
	if (e instanceof ApiError) {
		if (e.status === 410) {
			return 'This invite is used, has expired or is not valid. Ask the admin for a new link.';
		}
		if (e.status === 409) {
			return 'This invite cannot be used. The username is taken, or this server signs people in with Tailscale. Ask the admin for help.';
		}
		if (e.status === 429) {
			return 'Too many attempts with this invite. Wait an hour, then try again.';
		}
	}
	return errorMessage(e);
}
