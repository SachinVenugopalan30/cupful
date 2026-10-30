// SPDX-License-Identifier: AGPL-3.0-or-later
/** Shown when a write came back `stale` (a newer one already won); callers refetch instead of claiming success. */
export const staleNotice = (what: string) =>
	`A newer change to this ${what} was already saved on another device; showing the latest.`;
