// SPDX-License-Identifier: AGPL-3.0-or-later
// The small session cache: enough to boot straight into the last user's data offline, without
// ever showing one account's data to another.

import type { AuthState, Health, Profile } from '../api.ts';
import { deleteMeta, getMeta, putMeta } from './db.ts';

export type CachedSession = { userId: number; auth: AuthState; profile: Profile };

const ACTIVE_USER = 'active_user';
const authKey = (userId: number) => `auth:${userId}`;
const profileKey = (userId: number) => `profile:${userId}`;
const healthKey = (userId: number) => `health:${userId}`;

/** Marks `userId` active and stores just their auth state and profile. */
export async function saveSession(
	userId: number,
	auth: AuthState,
	profile: Profile
): Promise<void> {
	await putMeta(ACTIVE_USER, userId);
	await putMeta(authKey(userId), auth);
	await putMeta(profileKey(userId), profile);
}

/** The active user's cached session, or null when there is none (or it is inconsistent). */
export async function loadSessionCache(): Promise<CachedSession | null> {
	const userId = await getMeta<unknown>(ACTIVE_USER);
	if (typeof userId !== 'number' || !Number.isInteger(userId)) return null;

	const auth = await getMeta<AuthState>(authKey(userId));
	const profile = await getMeta<Profile>(profileKey(userId));
	if (!auth || !profile) return null;
	// The auth state's user must be the same account; otherwise refuse to boot as them.
	if (auth.user && auth.user.id !== userId) return null;

	return { userId, auth, profile };
}

/**
 * Drops one user's cached session. Also clears the active marker when it points at that user.
 * With no id, only the active marker is cleared.
 */
export async function clearSessionCache(userId?: number): Promise<void> {
	if (userId === undefined) {
		await deleteMeta(ACTIVE_USER);
		return;
	}
	await deleteMeta(authKey(userId));
	await deleteMeta(profileKey(userId));
	await deleteMeta(healthKey(userId));
	const active = await getMeta<unknown>(ACTIVE_USER);
	if (active === userId) await deleteMeta(ACTIVE_USER);
}

/** Caches the last health/version answer for the active user (Settings shows the version offline). */
export async function saveHealth(health: Health): Promise<void> {
	const userId = await getMeta<unknown>(ACTIVE_USER);
	if (typeof userId !== 'number') return;
	await putMeta(healthKey(userId), health);
}

export async function loadHealth(): Promise<Health | null> {
	const userId = await getMeta<unknown>(ACTIVE_USER);
	if (typeof userId !== 'number') return null;
	return (await getMeta<Health>(healthKey(userId))) ?? null;
}
