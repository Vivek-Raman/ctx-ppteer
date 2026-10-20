import test from 'node:test';
import assert from 'node:assert/strict';
import { startEventReadLifecycle } from '../src/shared/event-lifecycle.ts';

type SettingsValue = { value: number };

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

test('a newer event wins over a stale initial read', async () => {
  let emit: (value: SettingsValue) => void = () => undefined;
  const read = deferred<SettingsValue>();
  const seen: number[] = [];
  const lifecycle = startEventReadLifecycle({
    listen: async (onValue) => {
      emit = onValue;
      return () => undefined;
    },
    read: () => read.promise,
    onValue: (value) => seen.push(value.value),
    onError: assert.fail,
  });
  await Promise.resolve();
  emit({ value: 2 });
  read.resolve({ value: 1 });
  await lifecycle.ready;
  assert.deepEqual(seen, [2]);
});

test('initial read waits for listener installation', async () => {
  const listening = deferred<() => void>();
  let readStarted = false;
  const lifecycle = startEventReadLifecycle({
    listen: () => listening.promise,
    read: async () => {
      readStarted = true;
      return { value: 1 };
    },
    onValue: () => undefined,
    onError: assert.fail,
  });
  await Promise.resolve();
  assert.equal(readStarted, false);
  listening.resolve(() => undefined);
  await lifecycle.ready;
  assert.equal(readStarted, true);
});

test('dispose before registration cleans up and suppresses late work', async () => {
  const listening = deferred<() => void>();
  let stopped = false;
  const seen: number[] = [];
  const lifecycle = startEventReadLifecycle({
    listen: () => listening.promise,
    read: async () => ({ value: 1 }),
    onValue: (value) => seen.push(value.value),
    onError: assert.fail,
  });
  lifecycle.dispose();
  listening.resolve(() => {
    stopped = true;
  });
  await lifecycle.ready;
  assert.equal(stopped, true);
  assert.deepEqual(seen, []);
});

test('late read errors and listener errors are ignored after dispose', async () => {
  const read = deferred<SettingsValue>();
  let reportError: (error: unknown) => void = () => undefined;
  const errors: unknown[] = [];
  const lifecycle = startEventReadLifecycle({
    listen: async (_onValue, onError) => {
      reportError = onError;
      return () => undefined;
    },
    read: () => read.promise,
    onValue: () => undefined,
    onError: (error) => errors.push(error),
  });
  await Promise.resolve();
  lifecycle.dispose();
  reportError(new Error('late listener error'));
  read.reject(new Error('late read error'));
  await lifecycle.ready;
  assert.deepEqual(errors, []);
});

test('listener errors reach the live consumer', async () => {
  const error = new Error('listener failed');
  const errors: unknown[] = [];
  const lifecycle = startEventReadLifecycle({
    listen: async (_onValue, onError) => {
      onError(error);
      return () => undefined;
    },
    onValue: () => undefined,
    onError: (value) => errors.push(value),
  });
  await lifecycle.ready;
  assert.deepEqual(errors, [error]);
});
