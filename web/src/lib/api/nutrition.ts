// SPDX-License-Identifier: AGPL-3.0-or-later
// AI meal estimation. Logged data is written later through `sendOps`; this only asks for a draft.

import { api } from './http.ts';
import type { Estimate } from './types.ts';

/** POST /api/estimate. `photo` should already be re-encoded through a canvas (strips EXIF). */
export function estimate(description: string, photo?: Blob): Promise<Estimate> {
	const form = new FormData();
	form.set('description', description);
	if (photo) form.set('photo', photo, 'photo');
	return api.postForm<Estimate>('/api/estimate', form);
}
