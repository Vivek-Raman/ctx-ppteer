import test from 'node:test';
import assert from 'node:assert/strict';
import { ViewerController, type TimerApi } from '../src/viewer/controller.ts';
import type { Settings, Snapshot, StatusEntry } from '../src/shared/types.ts';

const settings: Settings = {
  theme: 'light',
  primaryColor: '#c98c5a',
  textScale: 1,
  pinned: false,
  projectSortOrder: 'project-directory-asc',
  autoClearAfterMinutes: null,
};
const entry: StatusEntry = {
  folder_name: 'demo',
  session_id: 's',
  branch: 'main',
  updated_at: '2026-10-19T00:00:00Z',
  points: [{ id: 'p1', text: 'one', done: false, link: null }],
};
const snapshot = (revision: number, entries = [entry]): Snapshot => ({
  path: '/tmp/status.md',
  document: { version: 2, entries },
  revision,
  health: 'current',
  error: null,
  modified_at: null,
});

function clock(reduced = false) {
  let now = 1_000_000;
  let next = 0;
  const timers = new Map<number, { due: number; fn: () => void }>();
  const api: TimerApi = {
    setTimeout: (fn, delay) => {
      const id = ++next;
      timers.set(id, { due: now + Math.max(0, delay), fn });
      return id as ReturnType<typeof setTimeout>;
    },
    clearTimeout: (id) => timers.delete(id as number),
    now: () => now,
    reducedMotion: () => reduced,
  };
  function advance(ms: number) {
    now += ms;
    let nextDue = [...timers.entries()].sort((a, b) => a[1].due - b[1].due)[0];
    while (nextDue && nextDue[1].due <= now) {
      timers.delete(nextDue[0]);
      nextDue[1].fn();
      nextDue = [...timers.entries()].sort((a, b) => a[1].due - b[1].due)[0];
    }
  }
  return { api, advance, pending: () => timers.size };
}

test('queued newest revision commits after the hold and notifies subscribers', () => {
  const fake = clock();
  const controller = new ViewerController(fake.api);
  const revisions: number[] = [];
  controller.subscribe((state) => revisions.push(state.revision));
  controller.acceptSettings(settings);
  controller.acceptSnapshot(snapshot(1));
  controller.setSelection(true);
  controller.acceptSnapshot(snapshot(2));
  controller.acceptSnapshot(snapshot(3));
  assert.equal(controller.state.revision, 1);
  assert.equal(controller.state.pendingSnapshot?.revision, 3);
  controller.setSelection(false);
  assert.equal(controller.state.revision, 1);
  fake.advance(600);
  assert.equal(controller.state.revision, 3);
  assert.ok(revisions.includes(3));
});

test('settings wait for layout motion and latest settings win', () => {
  const fake = clock();
  const controller = new ViewerController(fake.api);
  controller.acceptSettings(settings);
  controller.acceptSnapshot(snapshot(1));
  controller.acceptSettings({ ...settings, textScale: 1.15 });
  controller.acceptSettings({ ...settings, textScale: 1.35 });
  assert.equal(controller.state.settings?.textScale, 1);
  assert.equal(controller.state.pendingSettings?.textScale, 1.35);
  fake.advance(600);
  assert.equal(controller.state.settings?.textScale, 1.35);
});

test('selection release renders the queued snapshot', () => {
  const fake = clock();
  const controller = new ViewerController(fake.api);
  controller.acceptSettings(settings);
  controller.acceptSnapshot(snapshot(1));
  fake.advance(600);
  controller.setSelection(true);
  controller.acceptSnapshot(snapshot(2));
  controller.setSelection(false);
  assert.equal(controller.state.revision, 2);
});

test('dismissal marks the row before either request or exit settles', async () => {
  const fake = clock();
  const controller = new ViewerController(fake.api);
  controller.acceptSettings(settings);
  controller.acceptSnapshot(snapshot(1));
  fake.advance(600);
  let resolveRequest!: () => void;
  const request = new Promise<void>((resolve) => (resolveRequest = resolve));
  const dismissal = controller.dismiss(
    entry,
    entry.points[0],
    0,
    'row',
    async () => request,
    async () => snapshot(2),
  );
  assert.equal(controller.state.pendingRemoval, 'row');
  fake.advance(320);
  assert.equal(controller.state.pendingRemoval, 'row');
  resolveRequest();
  await dismissal;
  assert.equal(controller.state.pendingRemoval, '');
});

test('failed dismissal restores the row and records the error', async () => {
  const fake = clock();
  const controller = new ViewerController(fake.api);
  controller.acceptSettings(settings);
  controller.acceptSnapshot(snapshot(1));
  fake.advance(600);
  const dismissal = controller.dismiss(
    entry,
    entry.points[0],
    0,
    'row',
    async () => {
      throw new Error('offline');
    },
    async () => snapshot(2),
  );
  await dismissal;
  assert.equal(controller.state.pendingRemoval, '');
  assert.equal(controller.state.statusError, 'offline');
});

test('reduced motion uses zero deadlines and disposal clears timers and async dismissal', async () => {
  const fake = clock(true);
  const controller = new ViewerController(fake.api);
  controller.acceptSettings(settings);
  controller.acceptSnapshot(snapshot(1));
  assert.equal(controller.motionDuration(320), 0);
  let resolveRequest!: () => void;
  const request = new Promise<void>((resolve) => (resolveRequest = resolve));
  const dismissal = controller.dismiss(
    entry,
    entry.points[0],
    0,
    'row',
    async () => request,
    async () => snapshot(2),
  );
  controller.dispose();
  await dismissal;
  resolveRequest();
  assert.equal(fake.pending(), 0);
});

test('expiry refresh notifies age consumers', () => {
  const fake = clock();
  const controller = new ViewerController({ ...fake.api, now: () => 1_000_000 });
  const notifications: number[] = [];
  controller.subscribe((state) => notifications.push(state.now));
  controller.acceptSettings({ ...settings, autoClearAfterMinutes: 20 });
  controller.acceptSnapshot(snapshot(1));
  assert.ok(notifications.length > 2);
});
