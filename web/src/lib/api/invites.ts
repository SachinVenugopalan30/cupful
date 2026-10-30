// SPDX-License-Identifier: AGPL-3.0-or-later
// Invites: the admin list and creation, revocation, and the public acceptance call.

import { api, cachedGet } from './http.ts';
import type { Invite, NewInvite } from './types.ts';

export const invites = {
	/** Pending invites only. Cached for offline reading; never includes a token. */
	list: () => cachedGet<{ invites: Invite[] }>('/api/invites').then((r) => r.invites),
	/** The plain token is in the reply once; the server keeps only its hash. */
	create: () => api.post<NewInvite>('/api/invites'),
	remove: (id: number) => api.delete<object>(`/api/invites/${id}`),
	/** Public: creates the account and logs in. */
	accept: (b: { token: string; username: string; password: string; timezone: string }) =>
		api.post<{ id: number }>('/api/auth/accept-invite', b)
};
