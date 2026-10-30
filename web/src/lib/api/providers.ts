// SPDX-License-Identifier: AGPL-3.0-or-later
// Admin AI provider settings.

import { api } from './http.ts';
import type { Provider, ProviderIn, ProviderTest } from './types.ts';

export const providers = {
	list: () => api.get<{ providers: Provider[] }>('/api/ai/providers').then((r) => r.providers),
	create: (p: ProviderIn) => api.post<{ id: number }>('/api/ai/providers', p),
	update: (id: number, p: ProviderIn) => api.put<{ id: number }>(`/api/ai/providers/${id}`, p),
	remove: (id: number) => api.delete<object>(`/api/ai/providers/${id}`),
	/** Always 200 for an existing provider; read `ok`, `code` and `message`. */
	test: (id: number) => api.post<ProviderTest>(`/api/ai/providers/${id}/test`)
};
