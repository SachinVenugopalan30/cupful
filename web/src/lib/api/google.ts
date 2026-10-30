// SPDX-License-Identifier: AGPL-3.0-or-later
// Optional Google Health steps sync (404 unless the server is configured for it).

import { api } from './http.ts';
import type { GoogleStatus } from './types.ts';

export const google = {
	status: () => api.get<GoogleStatus>('/api/google'),
	/** Starts the Google sign-in; this is a page navigation (the server redirects to Google). */
	connectUrl: '/api/google/connect',
	sync: () => api.post<{ days: number; changed: number }>('/api/google/sync'),
	disconnect: () => api.delete<object>('/api/google')
};
