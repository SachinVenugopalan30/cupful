// SPDX-License-Identifier: AGPL-3.0-or-later
// The Activity screen's state: one day's steps, gym, trained parts and weigh-in, the 35-day window
// the week chart and workouts share, and the writes. Build it once in a component's script; it
// registers its effects there. Writes go only through #lib/sync (dayPatch + sendOps).

import { errorMessage, getDay, getDays, getWorkouts, google, type Day, type GoogleStatus } from '#lib/api.ts';
import { addDays } from '#lib/date.ts';
import { formatWeight, round, weightToKg } from '#lib/format.ts';
import { session, today } from '#lib/session.svelte.ts';
import { dayPatch, sendOps, type DayFields } from '#lib/sync/index.ts';
import { staleNotice } from '#lib/sync/stale.ts';
import { dataVersion } from '../offline/state.svelte.ts';
import {
	gymDates,
	lastTrained,
	lastWeight,
	sparklinePoints,
	stepsAverage,
	stepsVsTypical,
	toggle,
	weekWindow,
	weightDelta,
	windowCovers,
	type ActivityWindow,
	type Parsed
} from './logic.ts';

export type ActivityOptions = {
	date: string; // the selected day
	refreshKey: number; // bumped after any write to the day
	windowEnd?: string; // end of the fixed seven-day window; defaults to `date`
	syncSteps: boolean; // look up Google Health so the Steps card can show "Sync steps"
	onChanged: () => void; // a write was applied (the budget refreshes)
};

export class ActivityDay {
	data = $state<Day | null>(null);
	loadError = $state('');
	retry = $state(0);
	loading = $state(false);
	steps = $state('');
	gym = $state('');
	weight = $state('');
	trained = $state<string[]>([]);
	error = $state('');
	status = $state('');
	syncing = $state(false);
	syncNote = $state('');
	googleStatus = $state<GoogleStatus | null>(null);
	/** The day rows and workouts around the window end: one fetch for the cards, week and workouts. */
	win = $state<ActivityWindow | null>(null);

	#opts: () => ActivityOptions;

	constructor(opts: () => ActivityOptions) {
		this.#opts = opts;
		$effect(() => {
			if (!opts().syncSteps) return;
			google.status().then(
				(s) => (this.googleStatus = s),
				() => (this.googleStatus = null)
			);
		});
		$effect(() => {
			if (this.carried && this.weight === '') this.weight = this.shown(this.carried.kg);
		});
		let loaded = ''; // the date `data` belongs to
		$effect(() => {
			const { date: d, refreshKey, windowEnd } = opts();
			void refreshKey;
			void dataVersion.n;
			void this.retry;
			if (!d) return;
			// Keep the previous numbers and week on screen while the new day loads. Clearing them
			// made a date tap look like a full page reload.
			if (d !== loaded) this.loading = true;
			loaded = d;
			const end = windowEnd ?? d;
			let stale = false;
			this.loadError = '';
			// The window never blocks the form: if it fails, the cards simply show no extras. Workouts
			// fail separately (the endpoint may be empty or absent) and then read as none.
			Promise.all([
				getDays(addDays(end, -35), end).catch(() => [] as Day[]),
				getWorkouts(addDays(end, -6), end).catch(() => [])
			]).then(([days, workouts]) => !stale && (this.win = { date: end, days, workouts }));
			getDay(d).then(
				(r) => {
					if (stale) return;
					this.data = r;
					this.loading = false;
					this.steps = r.steps?.toString() ?? '';
					this.gym = r.gym_minutes?.toString() ?? '';
					this.weight = this.shown(r.weight_kg);
					this.trained = r.trained;
				},
				(e) => {
					if (stale) return;
					this.loadError = errorMessage(e);
					this.loading = false;
				}
			);
			return () => (stale = true);
		});
	}

	get date() {
		return this.#opts().date;
	}
	get unit() {
		return session.profile?.weight_unit ?? 'kg';
	}
	get typicalSteps() {
		return session.profile?.version?.typical_steps ?? null;
	}
	get future() {
		return this.date > today();
	}
	/** The window, once it is for this date. */
	get w() {
		return this.win && windowCovers(this.win, this.date, 30) ? this.win : null;
	}
	get weekDates() {
		return weekWindow(this.date);
	}
	get pct() {
		return this.data ? stepsVsTypical(this.data.steps, this.typicalSteps) : null;
	}
	get avg() {
		return this.w ? stepsAverage(this.w.days, this.weekDates) : null;
	}
	get sessions() {
		const gym = this.w ? gymDates(this.w.days, this.w.workouts) : null;
		return gym ? { gym, last: lastTrained(gym, this.date) } : null;
	}
	get delta() {
		return this.w ? weightDelta(this.w.days, this.date) : null;
	}
	get carried() {
		return this.data && this.data.weight_kg === null && this.w ? lastWeight(this.w.days, addDays(this.date, -1)) : null;
	}
	get trend() {
		const w = this.w;
		if (!w) return '';
		const values = Array.from({ length: 30 }, (_, i) => w.days.find((x) => x.date === addDays(this.date, i - 29))?.weight_kg ?? null);
		return sparklinePoints(values, 120, 32);
	}
	get canSync() {
		const g = this.googleStatus;
		return this.#opts().syncSteps && g?.configured === true && g.connected && !g.needs_reconnect && !this.future;
	}

	shown = (kg: number | null) => (kg === null ? '' : String(round(kg / weightToKg(1, this.unit), 1)));
	signed = (kg: number) => {
		const t = formatWeight(Math.abs(kg), this.unit);
		return Math.abs(round(kg / weightToKg(1, this.unit), 1)) === 0 ? 'No change' : `${kg < 0 ? '−' : '+'}${t}`;
	};

	async save(fields: DayFields, done: string): Promise<boolean> {
		this.error = this.status = this.syncNote = '';
		try {
			const [r] = await sendOps([dayPatch(this.date, fields)]);
			if (r.status === 'rejected') throw new Error(`Not saved: ${r.error ?? 'rejected'}.`);
			if (r.status === 'stale') {
				this.error = staleNotice('day');
				this.#opts().onChanged();
				return false;
			}
			this.status = r.status === 'queued' ? `${done} Waiting to sync.` : done;
			this.#opts().onChanged();
			return true;
		} catch (e) {
			this.error = errorMessage(e);
			return false;
		}
	}

	async syncFromGoogle() {
		this.syncing = true;
		this.error = this.status = this.syncNote = '';
		try {
			const r = await google.sync();
			this.status = r.changed > 0 ? 'Steps synced.' : 'Steps already up to date.';
			this.syncNote = this.status;
			this.#opts().onChanged();
		} catch (e) {
			this.error = errorMessage(e);
		} finally {
			this.syncing = false;
		}
	}

	/** Ticks a body part at once and saves the list; a failed save puts the saved list back. */
	async togglePart(part: string) {
		const on = this.trained.includes(part);
		this.trained = toggle(this.trained, part);
		if (!(await this.save({ trained: this.trained }, `${part} ${on ? 'removed' : 'added'}.`)) && this.data) {
			this.trained = this.data.trained;
		}
	}

	/** Saves a text field if it changed; a bad value shows its error and keeps what's typed. */
	commit(was: number | null, parsed: Parsed, key: keyof DayFields, done: string) {
		if ('error' in parsed) return void (this.error = parsed.error);
		if (parsed.value === was) return void (this.error = '');
		void this.save({ [key]: parsed.value }, done);
	}
}
