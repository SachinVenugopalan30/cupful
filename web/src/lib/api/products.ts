// SPDX-License-Identifier: AGPL-3.0-or-later
// The food catalog: foods, phrase aliases, portions, and packaged products.

import { api, cachedGet, enc } from './http.ts';
import type {
	Alias,
	AliasRow,
	Food,
	LabelIn,
	NewFood,
	Portion,
	Product,
	SavedProduct
} from './types.ts';

export const foods = {
	create: (f: NewFood) => api.post<Food>('/api/foods', f),
	alias: (alias: string, food_id: number) => api.post<Alias>('/api/aliases', { alias, food_id }),
	aliases: () => api.get<{ aliases: AliasRow[] }>('/api/aliases').then((r) => r.aliases),
	removeAlias: (id: number) => api.delete<object>(`/api/aliases/${id}`),
	portion: (phrase: string, food_id: number, grams: number) =>
		api.post<Portion>('/api/portions', { phrase, food_id, grams })
};

export const products = {
	/** By barcode. Errors: 400 bad barcode, product_not_found, lookup_failed, rate_limited. */
	lookup: (barcode: string) => api.get<Product>(`/api/products/${enc(barcode)}`),
	label: (b: LabelIn) => api.post<Product>('/api/products', b),
	list: () => cachedGet<{ products: SavedProduct[] }>('/api/products').then((r) => r.products),
	/** By food id. */
	remove: (foodId: number) => api.delete<object>(`/api/products/${foodId}`)
};
