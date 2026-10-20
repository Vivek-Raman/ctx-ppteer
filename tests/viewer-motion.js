// Run with npm run dev, then open /tests/viewer-motion.html.
// Exercises the actual Svelte viewer with an isolated Tauri IPC fixture.
import { mount, tick } from 'svelte';
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import Viewer from '../src/viewer/Viewer.svelte';
import '../src/styles.css';

const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const listeners = new Map();
let revision = 0;
let entries = [];
let settings = {
  theme: 'light',
  primaryColor: '#c98c5a',
  textScale: 1,
  projectSortOrder: 'project-directory-asc',
  autoClearAfterMinutes: null,
};
const snapshot = () => ({
  path: null,
  revision: ++revision,
  document: { version: 1, entries },
  health: 'current',
  error: null,
  modified_at: null,
});
const emit = (name, payload) =>
  window.__TAURI_INTERNALS__.runCallback(listeners.get(name), { payload });
mockWindows('main');
mockIPC((cmd, args) => {
  if (cmd === 'plugin:event|listen') {
    listeners.set(args.event, args.handler);
    return listeners.size;
  }
  if (cmd === 'get_settings') return settings;
  if (cmd === 'get_snapshot') return snapshot();
  if (cmd === 'dismiss_point') {
    entries = entries.flatMap((entry) => {
      if (entry.folder_name !== args.folderName || entry.session_id !== args.sessionId)
        return [entry];
      const points = entry.points.filter((_, index) => index !== args.pointIndex);
      return points.length ? [{ ...entry, points }] : [];
    });
    emit('source-update', snapshot());
  }
});
mount(Viewer, { target: document.querySelector('#app') });
const point = (text) => ({
  text,
  done: false,
  link: null,
  submitted_at: `2026-10-19T00:00:${String(point.next++).padStart(2, '0')}Z`,
});
point.next = 0;
const entry = (folder, session = folder, branch = 'main') => ({
  folder_name: folder,
  session_id: session,
  branch,
  updated_at: '2026-10-19T00:00:00Z',
  points: [point(`${session} point 0`), point(`${session} point 1`)],
});
const push = async (next) => {
  entries = next;
  emit('source-update', snapshot());
  await tick();
};
const find = (text) =>
  [...document.querySelectorAll('.point-text')].find((node) => node.textContent === text);
const assert = (condition, message) => {
  if (!condition) throw new Error(message);
};
const results = [];
async function motion(name, target, change, interrupted = false) {
  const before = target.getBoundingClientRect().top;
  const samples = [];
  const times = [];
  const changing = change();
  const start = performance.now();
  while (performance.now() - start < 1200) {
    await new Promise(requestAnimationFrame);
    samples.push(target.getBoundingClientRect().top);
    times.push(performance.now());
  }
  await changing;
  const after = target.getBoundingClientRect().top;
  const low = Math.min(before, after) - 1;
  const high = Math.max(before, after) + 1;
  assert(
    interrupted || samples.every((y) => y >= low && y <= high),
    `${name}: overshoot ${JSON.stringify({ before, after, samples })}`,
  );
  const direction = Math.sign(after - before);
  assert(
    interrupted || samples.every((y, i) => !i || direction * (y - samples[i - 1]) >= -1),
    `${name}: reversed direction`,
  );
  // Bound velocity rather than pixels per frame, so a busy browser may skip frames.
  const maxVelocity = (3 * Math.max(145, Math.abs(after - before))) / 300;
  assert(
    samples.every(
      (y, i) =>
        !i ||
        Math.abs(y - samples[i - 1]) <= 2 + maxVelocity * (times[i] - times[i - 1] + 17) * 1.25,
    ),
    `${name}: discontinuous motion ${JSON.stringify({ samples, times })}`,
  );
  assert(Math.abs(samples[0] - before) < 8, `${name}: jumped on first frame`);
  results.push({ name, before, after, frames: samples.length });
}
window.motionResults = (async () => {
  await pause(50);
  await push([entry('Alpha'), entry('Beta'), entry('Gamma')]);
  await pause(1300);
  await motion('insert point above another project', find('Gamma point 0'), () =>
    push(entries.map((e, i) => (i ? e : { ...e, points: [...e.points, point('New point')] }))),
  );
  await motion('insert sorted project', find('Gamma point 0'), () =>
    push([...entries, entry('Delta')]),
  );
  await motion('remove project', find('Gamma point 0'), () =>
    push(entries.filter((e) => e.folder_name !== 'Beta')),
  );
  await push([...entries, entry('Alpha', 'second-session')]);
  await pause(650);
  const retained = find('second-session point 0');
  await motion('remove earlier session in same branch', retained, () =>
    push(entries.filter((e) => e.session_id !== 'Alpha')),
  );
  assert(retained === find('second-session point 0'), 'session removal recreated surviving DOM');
  await push(
    entries.map((e, i) => ({ ...e, updated_at: new Date(Date.now() + i * 1000).toISOString() })),
  );
  await pause(650);
  await motion('reverse project sort', find('Gamma point 0'), async () => {
    settings = { ...settings, projectSortOrder: 'updated-at-asc' };
    emit('settings-update', settings);
    await tick();
  });
  await motion('newest first project sort', find('Gamma point 0'), async () => {
    settings = { ...settings, projectSortOrder: 'updated-at-desc' };
    emit('settings-update', settings);
    await tick();
  });
  await motion('rapid insertion while moving', find('second-session point 0'), async () => {
    await push(
      [...entries, entry('Newest')].map((e, i) => ({
        ...e,
        updated_at: new Date(Date.now() + i * 1000).toISOString(),
      })),
    );
    await pause(90);
    await push(
      [...entries, entry('Even newer')].map((e, i) => ({
        ...e,
        updated_at: new Date(Date.now() + i * 1000).toISOString(),
      })),
    );
  });
  await motion(
    'remove project during reflow',
    find('second-session point 0'),
    async () => {
      await push(
        [...entries, entry('Temporary')].map((e, i) => ({
          ...e,
          updated_at: new Date(Date.now() + i * 1000).toISOString(),
        })),
      );
      await pause(90);
      await push(entries.filter((e) => e.folder_name !== 'Even newer'));
    },
    true,
  );
  const sameTime = entry('Same timestamp');
  sameTime.points = [point('Same time A'), point('Same time B'), point('Same time B')].map((p) => ({
    ...p,
    submitted_at: '2026-10-19T00:00:00Z',
  }));
  await push([...entries, sameTime]);
  await pause(650);
  assert(
    document.querySelectorAll('.point-text').length ===
      entries.reduce((n, e) => n + e.points.length, 0),
    'timestamp collision lost rows',
  );
  results.push({ name: 'shared timestamps and identical points have unique keys' });
  const dismissed = find('second-session point 0');
  const below = find('second-session point 1');
  const oldY = below.getBoundingClientRect().top;
  dismissed.closest('li').querySelector('button').click();
  await pause(180);
  assert(dismissed.isConnected, 'dismissal removed row before exit completed');
  assert(
    Math.abs(below.getBoundingClientRect().top - oldY) < 1,
    'dismissal moved survivors before exit completed',
  );
  await pause(650);
  assert(!dismissed.isConnected, 'dismissed row still present');
  results.push({ name: 'dismissal waits for exit before reflow' });
  await pause(250);
  const matchMedia = window.matchMedia.bind(window);
  window.matchMedia = (query) =>
    query === '(prefers-reduced-motion: reduce)' ? { matches: true } : matchMedia(query);
  await push([...entries, entry('Reduced motion')]);
  await pause(80);
  const reduced = find('Reduced motion point 0');
  assert(
    reduced && getComputedStyle(reduced.closest('.motion-row').firstElementChild).opacity === '1',
    'reduced motion delayed entry',
  );
  results.push({ name: 'reduced motion skips delays and animation' });
  return { passed: results.length, results };
})().catch((error) => ({ error: error.stack, results }));

window.motionResults.then((result) => {
  document.title = result.error
    ? 'FAIL: Viewer motion'
    : `PASS: ${result.passed} viewer motion checks`;
  console.log(result);
});
