// SPDX-License-Identifier: AGPL-3.0-or-later
// Shortcut/ingest tokens for POST /api/ingest/activity.

import { api } from './http.ts';
import type { IngestToken } from './types.ts';

export const ingestTokens = {
	list: () => api.get<{ tokens: IngestToken[] }>('/api/ingest-tokens').then((r) => r.tokens),
	/** The plain token is in the reply once; the server keeps only a hash. */
	create: (label: string) =>
		api.post<IngestToken & { token: string }>('/api/ingest-tokens', { label }),
	remove: (id: number) => api.delete<object>(`/api/ingest-tokens/${id}`)
};
