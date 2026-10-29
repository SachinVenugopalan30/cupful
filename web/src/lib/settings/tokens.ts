// SPDX-License-Identifier: AGPL-3.0-or-later
// The curl line shown next to a new ingest token (PLAN.md §11).

/** A curl command that posts 5000 steps for `date` with `token`. */
export function curlExample(origin: string, token: string, date = new Date().toISOString().slice(0, 10)): string {
	return [
		`curl -X POST ${origin}/api/ingest/activity`,
		`  -H 'Authorization: Bearer ${token}'`,
		`  -H 'Content-Type: application/json'`,
		`  -d '{"date":"${date}","steps":5000}'`
	].join(' \\\n');
}
