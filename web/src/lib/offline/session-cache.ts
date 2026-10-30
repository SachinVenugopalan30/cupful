// SPDX-License-Identifier: AGPL-3.0-or-later
// The small session cache: enough to boot straight into the last user's data offline, without
// ever showing one account's data to another.

import type { AuthState, Health, Profile } from '../api.ts';
import { deleteMeta, getMeta, putMeta } from './db.ts';

export type CachedSession = { userId: number; auth: AuthState; profile: Profile };

const ACTIVE_USER = 'active_user';
const PREVIOUS_USER = 'previous_user';
const authKey = (userId: number) => `auth:${userId}`;
const profileKey = (userId: number) => `profile:${userId}`;
const healthKey = (userId: number) => `health:${userId}`;

/** The active user id, or null when none is stored (or the marker is not an integer). */
async function activeUserId(): Promise<number | null> {
	const userId = await getMeta<unknown>(ACTIVE_USER);
	return typeof userId === 'number' && Number.isInteger(userId) ? userId : null;
}

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
	const userId = await activeUserId();
	if (userId === null) return null;

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
	const userId = await activeUserId();
	if (userId === null) return;
	await putMeta(healthKey(userId), health);
}

export async function loadHealth(): Promise<Health | null> {
	const userId = await activeUserId();
	if (userId === null) return null;
	return (await getMeta<Health>(healthKey(userId))) ?? null;
}

/** The account that was signed in before the server session ended, kept to warn on a switch. */
export type PreviousUser = { userId: number; username: string };

export async function savePreviousUser(userId: number, username: string): Promise<void> {
	await putMeta(PREVIOUS_USER, { userId, username });
}

export async function loadPreviousUser(): Promise<PreviousUser | null> {
	const value = await getMeta<PreviousUser>(PREVIOUS_USER);
	if (!value || typeof value.userId !== 'number' || typeof value.username !== 'string') return null;
	return value;
}

export async function clearPreviousUser(): Promise<void> {
	await deleteMeta(PREVIOUS_USER);
}
