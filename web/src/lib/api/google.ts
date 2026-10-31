// SPDX-License-Identifier: AGPL-3.0-or-later
// Optional Google Health steps sync (404 unless the server is configured for it).

import { api } from './http.ts';
import type { GoogleStatus } from './types.ts';

export const google = {
	status: () => api.get<GoogleStatus>('/api/google'),
	/** Starts the Google sign-in; this is a page navigation (the server redirects to Google). */
	connectUrl: '/api/google/connect',
	sync: () =>
		api.post<{ days: number; changed: number; workouts: number; workouts_error?: string }>(
			'/api/google/sync'
		),
	/** Which workout types count as gym minutes; the server recomputes the gym minutes. */
	setGymTypes: (types: string[]) => api.put<object>('/api/google/gym-types', { types }),
	disconnect: () => api.delete<object>('/api/google')
};
