// SPDX-License-Identifier: AGPL-3.0-or-later
import type { GoogleStatus } from '#lib/api.ts';

export type GoogleView = 'hidden' | 'connect' | 'connected' | 'reconnect';

/** Which state the Google Health card shows. Unconfigured instances hide the card entirely. */
export function googleView(s: GoogleStatus | null): GoogleView {
	if (!s || !s.configured) return 'hidden';
	if (!s.connected) return 'connect';
	return s.needs_reconnect ? 'reconnect' : 'connected';
}

/** The `?google=` result Google's redirect leaves on the Settings URL. */
export function googleResult(search: string): { ok: boolean; message: string } | null {
	switch (new URLSearchParams(search).get('google')) {
		case 'connected':
			return { ok: true, message: 'Google Health connected.' };
		case 'denied':
			return { ok: false, message: 'Google Health was not connected: access was declined.' };
		case 'error':
			return { ok: false, message: 'Google Health could not be connected. Try again.' };
		default:
			return null;
	}
}
