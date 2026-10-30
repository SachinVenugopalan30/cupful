// SPDX-License-Identifier: AGPL-3.0-or-later
// Account configuration: sign-in state and the profile.

import { api, cachedGet } from './http.ts';
import type { AuthState, Profile, ProfileIn } from './types.ts';

export const auth = {
	state: () => api.get<AuthState>('/api/auth/state'),
	setup: (b: { username: string; password: string; timezone: string }) =>
		api.post<{ id: number }>('/api/auth/setup', b),
	login: (b: { username: string; password: string }) => api.post<{ id: number }>('/api/auth/login', b),
	logout: () => api.post<object>('/api/auth/logout')
};

export const profile = {
	get: () => cachedGet<Profile>('/api/profile'),
	save: (p: ProfileIn) => api.put<object>('/api/profile', p)
};
