import test from 'node:test';
import assert from 'node:assert/strict';
import { ViewerController, type TimerApi, type ViewerMotion } from '../src/viewer/controller.ts';
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

test('queued newest revision commits after the hold and notifies subscribers', async () => {
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
  await microtasks();
  assert.equal(controller.state.revision, 3);
  assert.ok(revisions.includes(3));
});

test('settings wait for layout motion and latest settings win', async () => {
  const fake = clock();
  const controller = new ViewerController(fake.api);
  controller.acceptSettings(settings);
  controller.acceptSnapshot(snapshot(1));
  controller.acceptSettings({ ...settings, textScale: 1.15 });
  controller.acceptSettings({ ...settings, textScale: 1.35 });
  assert.equal(controller.state.settings?.textScale, 1);
  assert.equal(controller.state.pendingSettings?.textScale, 1.35);
  fake.advance(600);
  await microtasks();
  assert.equal(controller.state.settings?.textScale, 1.35);
});

test('selection release renders the queued snapshot', async () => {
  const fake = clock();
  const controller = new ViewerController(fake.api);
  controller.acceptSettings(settings);
  controller.acceptSnapshot(snapshot(1));
  fake.advance(600);
  await microtasks();
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
  await microtasks();
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
  assert.equal(controller.state.pendingRemoval.has('row'), true);
  fake.advance(320);
  await microtasks();
  assert.equal(controller.state.pendingRemoval.has('row'), true);
  resolveRequest();
  await dismissal;
  assert.equal(controller.state.pendingRemoval.size, 0);
});

test('failed dismissal restores the row and records the error', async () => {
  const fake = clock();
  const controller = new ViewerController(fake.api);
  controller.acceptSettings(settings);
  controller.acceptSnapshot(snapshot(1));
  fake.advance(600);
  await microtasks();
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
  assert.equal(controller.state.pendingRemoval.size, 0);
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

test('expiry refresh notifies age consumers', async () => {
  const fake = clock();
  const controller = new ViewerController({ ...fake.api, now: () => 1_000_000 });
  const notifications: number[] = [];
  controller.subscribe((state) => notifications.push(state.now));
  controller.acceptSettings({ ...settings, autoClearAfterMinutes: 20 });
  controller.acceptSnapshot(snapshot(1));
  assert.ok(notifications.length > 2);
});

function deferred<T = void>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
const microtasks = async () => {
  for (let i = 0; i < 15; i++) await Promise.resolve();
};

test('parallel dismissals hold snapshots and settings until the last exit and request', async () => {
  const fake = clock();
  const controller = new ViewerController(fake.api);
  controller.acceptSettings(settings);
  controller.acceptSnapshot(snapshot(1));
  fake.advance(600);
  await microtasks();
  const firstRequest = deferred();
  const secondRequest = deferred();
  const first = controller.dismiss(
    entry,
    entry.points[0],
    0,
    'first',
    () => firstRequest.promise,
    async () => snapshot(2),
  );
  fake.advance(100);
  await microtasks();
  const second = controller.dismiss(
    entry,
    entry.points[0],
    0,
    'second',
    () => secondRequest.promise,
    async () => snapshot(4, []),
  );
  controller.acceptSettings({ ...settings, textScale: 1.35 });
  controller.acceptSnapshot(snapshot(3));
  firstRequest.resolve();
  fake.advance(220);
  await microtasks();
  await first;
  assert.equal(controller.state.revision, 1);
  assert.equal(controller.state.settings?.textScale, 1);
  assert.deepEqual([...controller.state.pendingRemoval], ['first', 'second']);
  fake.advance(100);
  await microtasks();
  await microtasks();
  assert.equal(controller.state.revision, 1, 'the last request still holds layout');
  secondRequest.resolve();
  await second;
  assert.equal(controller.state.revision, 4);
  assert.equal(controller.state.settings?.textScale, 1.35);
  assert.equal(controller.state.pendingRemoval.size, 0);
});

test('new dismissals join the active group and duplicate clicks are ignored', async () => {
  const fake = clock();
  const controller = new ViewerController(fake.api);
  controller.acceptSnapshot(snapshot(1));
  fake.advance(600);
  await microtasks();
  let requests = 0;
  const run = async () => {
    requests++;
  };
  const first = controller.dismiss(entry, entry.points[0], 0, 'first', run, async () =>
    snapshot(2),
  );
  await controller.dismiss(entry, entry.points[0], 0, 'first', run, async () => snapshot(2));
  fake.advance(300);
  await microtasks();
  const second = controller.dismiss(entry, entry.points[0], 0, 'second', run, async () =>
    snapshot(3),
  );
  fake.advance(20);
  await microtasks();
  await first;
  assert.equal(requests, 2);
  assert.equal(controller.state.revision, 1);
  await controller.dismiss(entry, entry.points[0], 0, 'first', run, async () => snapshot(2));
  assert.equal(requests, 2, 'completed exits cannot be dismissed twice');
  fake.advance(300);
  await microtasks();
  await second;
  assert.equal(controller.state.revision, 3);
});

test('dismissals take priority over queued layout after existing motion finishes', async () => {
  const fake = clock();
  const controller = new ViewerController(fake.api);
  controller.acceptSnapshot(snapshot(1));
  controller.acceptSnapshot(snapshot(2));
  let started = false;
  const dismissal = controller.dismiss(
    entry,
    entry.points[0],
    0,
    'row',
    async () => {
      started = true;
    },
    async () => snapshot(3),
  );
  assert.equal(started, false);
  fake.advance(600);
  await microtasks();
  assert.equal(started, true);
  assert.equal(controller.state.revision, 1);
  fake.advance(320);
  await microtasks();
  await dismissal;
  assert.equal(controller.state.revision, 3);
});

test('a failed dismissal releases only its own row while another request is active', async () => {
  const fake = clock();
  const controller = new ViewerController(fake.api);
  controller.acceptSnapshot(snapshot(1));
  fake.advance(600);
  await microtasks();
  const request = deferred();
  const first = controller.dismiss(
    entry,
    entry.points[0],
    0,
    'first',
    () => request.promise,
    async () => snapshot(2),
  );
  const failed = controller.dismiss(
    entry,
    entry.points[0],
    0,
    'failed',
    async () => {
      throw new Error('offline');
    },
    async () => snapshot(3),
  );
  await failed;
  assert.deepEqual([...controller.state.pendingRemoval], ['first']);
  assert.equal(controller.state.statusError, 'offline');
  assert.equal(controller.state.revision, 1);
  request.resolve();
  fake.advance(320);
  await microtasks();
  await first;
  assert.equal(controller.state.revision, 2);
  assert.equal(
    controller.state.statusError,
    'offline',
    'another dismissal must not erase the failure',
  );
});

test('late snapshot reads cannot replace a newer dismissal result', async () => {
  const fake = clock();
  const controller = new ViewerController(fake.api);
  controller.acceptSnapshot(snapshot(1));
  fake.advance(600);
  await microtasks();
  const oldRead = deferred<Snapshot>();
  const first = controller.dismiss(
    entry,
    entry.points[0],
    0,
    'first',
    async () => {},
    () => oldRead.promise,
  );
  const second = controller.dismiss(
    entry,
    entry.points[0],
    0,
    'second',
    async () => {},
    async () => snapshot(4, []),
  );
  fake.advance(320);
  await microtasks();
  await second;
  assert.equal(controller.state.revision, 1);
  oldRead.resolve(snapshot(2));
  await first;
  assert.equal(controller.state.revision, 4);
});

test('selection continues holding layout after dismissal completion', async () => {
  const fake = clock();
  const controller = new ViewerController(fake.api);
  controller.acceptSnapshot(snapshot(1));
  fake.advance(600);
  await microtasks();
  controller.setSelection(true);
  const dismissal = controller.dismiss(
    entry,
    entry.points[0],
    0,
    'row',
    async () => {},
    async () => snapshot(2, []),
  );
  fake.advance(320);
  await microtasks();
  await dismissal;
  assert.equal(controller.state.revision, 1);
  assert.equal(controller.state.pendingRemoval.has('row'), true);
  controller.setSelection(false);
  assert.equal(controller.state.revision, 2);
});

test('disposal releases queued dismissals and ignores late snapshot reads', async () => {
  const fake = clock();
  const controller = new ViewerController(fake.api);
  controller.acceptSnapshot(snapshot(1));
  let requests = 0;
  const queued = controller.dismiss(
    entry,
    entry.points[0],
    0,
    'queued',
    async () => {
      requests++;
    },
    async () => snapshot(2),
  );
  controller.dispose();
  await queued;
  assert.equal(requests, 0);
  assert.equal(fake.pending(), 0);

  const other = new ViewerController(fake.api);
  const read = deferred<Snapshot>();
  const active = other.dismiss(
    entry,
    entry.points[0],
    0,
    'active',
    async () => {},
    () => read.promise,
  );
  fake.advance(320);
  await microtasks();
  await microtasks();
  other.dispose();
  await active;
  read.resolve(snapshot(3));
  await microtasks();
  assert.equal(other.state.revision, -1);
  assert.equal(fake.pending(), 0);
});

test('a failed snapshot read restores the row and releases the animation lane', async () => {
  const fake = clock();
  const controller = new ViewerController(fake.api);
  controller.acceptSnapshot(snapshot(1));
  fake.advance(600);
  await microtasks();
  const dismissal = controller.dismiss(
    entry,
    entry.points[0],
    0,
    'row',
    async () => {},
    async () => {
      throw new Error('read failed');
    },
  );
  fake.advance(320);
  await microtasks();
  await dismissal;
  assert.equal(controller.state.pendingRemoval.size, 0);
  assert.equal(controller.state.dismissing.size, 0);
  assert.equal(controller.state.statusError, 'read failed');
  controller.acceptSnapshot(snapshot(2, []));
  assert.equal(controller.state.revision, 2);
});

function controlledMotion() {
  const calls: string[] = [];
  const exits: ReturnType<typeof deferred>[] = [];
  const moves: ReturnType<typeof deferred>[] = [];
  const entrances: ReturnType<typeof deferred>[] = [];
  const dismissals: ReturnType<typeof deferred>[] = [];
  const motion: ViewerMotion = {
    exit: () => {
      calls.push('exit');
      const wait = deferred();
      exits.push(wait);
      return wait.promise;
    },
    move: (commit) => {
      calls.push('move');
      commit();
      const wait = deferred();
      moves.push(wait);
      return wait.promise;
    },
    enter: () => {
      calls.push('enter');
      const wait = deferred();
      entrances.push(wait);
      return wait.promise;
    },
    dismiss: async () => {
      calls.push('dismiss');
      const wait = deferred();
      dismissals.push(wait);
      await wait.promise;
    },
    restore: () => {
      calls.push('restore');
    },
    dispose: () => {
      calls.push('dispose');
      [...exits, ...moves, ...entrances, ...dismissals].forEach((wait) => wait.resolve());
    },
  };
  return { motion, calls, exits, moves, entrances, dismissals };
}

test('browser completion signals gate exits, reorders, and entrances individually', async () => {
  const fake = clock();
  const view = controlledMotion();
  const controller = new ViewerController(fake.api, view.motion);
  controller.acceptSnapshot(snapshot(1));
  assert.deepEqual(view.calls, ['exit']);
  fake.advance(10_000);
  await microtasks();
  assert.equal(controller.state.revision, -1, 'elapsed time cannot finish an active animation');
  view.exits[0].resolve();
  await microtasks();
  assert.deepEqual(view.calls, ['exit', 'move']);
  assert.equal(controller.state.revision, 1);
  view.moves[0].resolve();
  await microtasks();
  assert.deepEqual(view.calls, ['exit', 'move', 'enter']);
  controller.acceptSnapshot(snapshot(2));
  await microtasks();
  assert.equal(view.exits.length, 1, 'new updates wait for the active entrance');
  view.entrances[0].resolve();
  await microtasks();
  assert.equal(view.exits.length, 2);
  controller.dispose();
});

test('updates during exit restart the exit phase with the latest target before reordering', async () => {
  const view = controlledMotion();
  const controller = new ViewerController(clock().api, view.motion);
  controller.acceptSnapshot(snapshot(1));
  controller.acceptSnapshot(snapshot(3, []));
  controller.acceptSnapshot(snapshot(2));
  view.exits[0].resolve();
  await microtasks();
  assert.deepEqual(view.calls, ['exit', 'exit']);
  assert.equal(controller.state.revision, -1);
  view.exits[1].resolve();
  await microtasks();
  assert.equal(controller.state.revision, 3);
  assert.deepEqual(view.calls, ['exit', 'exit', 'move']);
  controller.dispose();
});

test('a dismissal joins exits and blocks reordering until its animation and request complete', async () => {
  const view = controlledMotion();
  const controller = new ViewerController(clock().api, view.motion);
  controller.acceptSnapshot(snapshot(1));
  const request = deferred();
  const dismissal = controller.dismiss(
    entry,
    entry.points[0],
    0,
    'row',
    () => request.promise,
    async () => snapshot(2, []),
  );
  assert.deepEqual(view.calls, ['exit', 'dismiss']);
  view.exits[0].resolve();
  request.resolve();
  await microtasks();
  assert.equal(view.moves.length, 0);
  view.dismissals[0].resolve();
  await dismissal;
  await microtasks();
  assert.deepEqual(view.calls, ['exit', 'dismiss', 'exit']);
  view.exits[1].resolve();
  await microtasks();
  assert.equal(view.moves.length, 1);
  assert.equal(controller.state.revision, 2);
  controller.dispose();
});

test('queued dismissals run before entrances and selection never blocks a dismissal', async () => {
  const view = controlledMotion();
  const controller = new ViewerController(clock().api, view.motion);
  controller.acceptSnapshot(snapshot(1));
  view.exits[0].resolve();
  await microtasks();
  const dismissal = controller.dismiss(
    entry,
    entry.points[0],
    0,
    'row',
    async () => {},
    async () => snapshot(2),
  );
  controller.setSelection(true);
  view.moves[0].resolve();
  await microtasks();
  assert.deepEqual(view.calls, ['exit', 'move', 'dismiss']);
  view.dismissals[0].resolve();
  await dismissal;
  assert.equal(view.entrances.length, 0);
  controller.setSelection(false);
  await microtasks();
  assert.equal(view.entrances.length, 0, 'new exits have priority over stale entrances');
  assert.equal(view.exits.length, 2);
  view.exits[1].resolve();
  await microtasks();
  view.moves[1].resolve();
  await microtasks();
  assert.equal(view.entrances.length, 1);
  controller.dispose();
});

test('animation failures release the phase and disposal ignores late completions', async () => {
  const view = controlledMotion();
  const controller = new ViewerController(clock().api, view.motion);
  controller.acceptSnapshot(snapshot(1));
  view.exits[0].reject(new Error('animation failed'));
  await microtasks();
  assert.equal(view.moves.length, 1);
  controller.dispose();
  await microtasks();
  assert.equal(view.entrances.length, 0);
});
