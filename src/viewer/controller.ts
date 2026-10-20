import type { Settings, Snapshot, StatusEntry, StatusPoint } from '../shared/types';

export type ViewerState = {
  revision: number;
  entries: StatusEntry[];
  settings: Settings | null;
  pendingSnapshot: Snapshot | null;
  pendingSettings: Settings | null;
  selectedText: boolean;
  pendingRemoval: Set<string>;
  dismissing: Set<string>;
  statusError: string;
  now: number;
};
export type TimerApi = {
  setTimeout(handler: () => void, delay: number): ReturnType<typeof setTimeout>;
  clearTimeout(id: ReturnType<typeof setTimeout>): void;
  now(): number;
  reducedMotion(): boolean;
};
export type ViewerLayout = { entries: StatusEntry[]; settings: Settings | null };
export interface ViewerMotion {
  exit(layout: ViewerLayout): void | Promise<void>;
  move(commit: () => void): void | Promise<void>;
  enter(): void | Promise<void>;
  dismiss(key: string): Promise<void>;
  restore(key: string): void;
  dispose(): void;
}
type Listener = (state: ViewerState) => void;
type DismissArgs = {
  folderName: string;
  sessionId: string | null;
  branch: string;
  updatedAt: string;
  pointId: string | null;
  pointIndex: number | null;
};
const initialState = (): ViewerState => ({
  revision: -1,
  entries: [],
  settings: null,
  pendingSnapshot: null,
  pendingSettings: null,
  selectedText: false,
  pendingRemoval: new Set(),
  dismissing: new Set(),
  statusError: '',
  now: Date.now(),
});

// Dismissals and row exits share the highest priority and can run together.
// Reordering and entrances each wait for the preceding group to finish.
type AnimationTask = {
  key: string;
  priority: number;
  ready: () => boolean;
  run: (complete: () => void) => void;
};
class AnimationQueue {
  private pending: AnimationTask[] = [];
  private active = new Map<string, number>();
  private draining = false;
  private disposed = false;

  enqueue(task: AnimationTask) {
    if (this.disposed || this.pending.some(({ key }) => key === task.key)) return;
    this.pending.push(task);
    this.drain();
  }
  drain() {
    if (this.disposed || this.draining) return;
    this.draining = true;
    try {
      while (!this.disposed) {
        this.pending.sort((a, b) => a.priority - b.priority);
        const index = this.pending.findIndex((task) => task.ready());
        const task = this.pending[index];
        if (!task) break;
        if (
          this.active.size &&
          (task.priority !== 0 || [...this.active.values()].some((priority) => priority !== 0))
        )
          break;
        this.pending.splice(index, 1);
        this.active.set(task.key, task.priority);
        let completed = false;
        task.run(() => {
          if (completed) return;
          completed = true;
          this.active.delete(task.key);
          this.drain();
        });
      }
    } finally {
      this.draining = false;
    }
  }
  dispose() {
    this.disposed = true;
    this.pending = [];
    this.active.clear();
  }
}

export class ViewerController {
  readonly state: ViewerState = initialState();
  private readonly animations = new AnimationQueue();
  private readonly motion?: ViewerMotion;
  private layoutPending = false;
  private motionTimer: ReturnType<typeof setTimeout> | undefined;
  private expiryTimer: ReturnType<typeof setTimeout> | undefined;
  private disposed = false;
  private pendingDismissalError: string | null = null;
  private readonly exitDuration = 320;
  private readonly timers: TimerApi;
  private readonly listeners = new Set<Listener>();
  private readonly teardownWaiters = new Set<() => void>();
  private readonly dismissalTimers = new Set<ReturnType<typeof setTimeout>>();
  private notifying = false;
  private notifyAgain = false;
  constructor(timers: TimerApi, motion?: ViewerMotion) {
    this.timers = timers;
    this.motion = motion;
    this.state.now = timers.now();
  }
  subscribe(listener: Listener): () => void {
    if (this.disposed) return () => undefined;
    listener(this.snapshot());
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
  snapshot(): ViewerState {
    return {
      ...this.state,
      entries: [...this.state.entries],
      pendingRemoval: new Set(this.state.pendingRemoval),
      dismissing: new Set(this.state.dismissing),
      settings: this.state.settings ? { ...this.state.settings } : null,
      pendingSnapshot: this.state.pendingSnapshot,
      pendingSettings: this.state.pendingSettings ? { ...this.state.pendingSettings } : null,
    };
  }
  private notify() {
    if (this.disposed) return;
    if (this.notifying) {
      this.notifyAgain = true;
      return;
    }
    do {
      this.notifyAgain = false;
      this.notifying = true;
      const next = this.snapshot();
      this.listeners.forEach((listener) => listener(next));
      this.notifying = false;
    } while (this.notifyAgain && !this.disposed);
  }
  private setError(error: unknown) {
    if (this.disposed) return;
    this.state.statusError = error instanceof Error ? error.message : String(error);
    this.notify();
  }
  setErrorForView(error: unknown) {
    this.setError(error);
  }
  motionDuration(duration: number) {
    return this.timers.reducedMotion() ? 0 : duration;
  }
  acceptSettings(value: Settings) {
    if (this.disposed) return;
    this.state.pendingSettings = value;
    this.flushPending();
  }
  acceptSnapshot(value: Snapshot) {
    if (
      this.disposed ||
      value.revision <= this.state.revision ||
      (this.state.pendingSnapshot && value.revision < this.state.pendingSnapshot.revision)
    )
      return;
    this.state.pendingSnapshot = value;
    this.flushPending();
  }
  setSelection(selected: boolean) {
    if (this.disposed) return;
    this.state.selectedText = selected;
    this.notify();
    if (!selected) this.flushPending();
  }
  refreshNow() {
    if (!this.disposed) {
      this.state.now = this.timers.now();
      this.scheduleExpiry();
      this.notify();
    }
  }
  private commitPending() {
    const settings = this.state.pendingSettings;
    const snapshot = this.state.pendingSnapshot;
    if (settings) this.state.settings = { ...settings };
    if (snapshot) {
      this.state.revision = snapshot.revision;
      this.state.entries = snapshot.document.entries;
      this.state.statusError = snapshot.error ?? this.pendingDismissalError ?? '';
      this.pendingDismissalError = null;
    }
    this.state.pendingSettings = null;
    this.state.pendingSnapshot = null;
    this.state.pendingRemoval.clear();
    this.scheduleExpiry();
    this.notify();
  }
  private phase(key: string, priority: number, run: () => void | Promise<void>, next: () => void) {
    this.animations.enqueue({
      key,
      priority,
      ready: () => !this.state.selectedText,
      run: (complete) => {
        const finish = () => {
          if (!this.disposed) next();
          complete();
        };
        try {
          const result = run();
          if (result) void result.catch((error) => this.setError(error)).then(finish);
          else finish();
        } catch (error) {
          this.setError(error);
          finish();
        }
      },
    });
  }
  private flushPending() {
    if (this.disposed) return;
    if (!this.layoutPending && (this.state.pendingSettings || this.state.pendingSnapshot)) {
      this.layoutPending = true;
      // Preparation waits for the dismissal group before choosing what to exit.
      this.phase(
        'prepare',
        1,
        () => undefined,
        () => this.queueExits(),
      );
    }
    this.animations.drain();
  }
  private queueExits() {
    const snapshot = this.state.pendingSnapshot;
    const settings = this.state.pendingSettings;
    this.phase(
      'exit',
      0,
      () =>
        this.motion?.exit({
          entries: snapshot?.document.entries ?? this.state.entries,
          settings: settings ?? this.state.settings,
        }),
      () => {
        let committed = false;
        this.phase(
          'move',
          1,
          () => {
            // A dismissal or update may have arrived during the exit phase. Fade
            // any additional outgoing rows before committing the latest layout.
            if (snapshot !== this.state.pendingSnapshot || settings !== this.state.pendingSettings)
              return;
            committed = true;
            if (this.motion)
              return this.motion.move(() => {
                if (!this.disposed) this.commitPending();
              });
            const layoutChanged =
              settings &&
              this.state.settings &&
              (settings.projectSortOrder !== this.state.settings.projectSortOrder ||
                settings.textScale !== this.state.settings.textScale);
            this.commitPending();
            if (snapshot || layoutChanged)
              return new Promise<void>((resolve) => {
                this.motionTimer = this.timers.setTimeout(() => {
                  this.motionTimer = undefined;
                  this.teardownWaiters.delete(resolve);
                  resolve();
                }, this.motionDuration(600));
                this.teardownWaiters.add(resolve);
              });
          },
          () => {
            if (!committed) {
              this.queueExits();
              return;
            }
            let entered = false;
            this.phase(
              'enter',
              2,
              () => {
                if (this.state.pendingSnapshot || this.state.pendingSettings) return;
                entered = true;
                return this.motion?.enter();
              },
              () => {
                if (!entered) {
                  this.queueExits();
                  return;
                }
                this.layoutPending = false;
                this.flushPending();
              },
            );
          },
        );
      },
    );
  }
  expirySoon(value: string) {
    const minutes = this.state.settings?.autoClearAfterMinutes;
    return (
      minutes != null &&
      new Date(value).getTime() + minutes * 60_000 - this.state.now <= 10 * 60_000
    );
  }
  private scheduleExpiry() {
    if (this.expiryTimer !== undefined) this.timers.clearTimeout(this.expiryTimer);
    this.expiryTimer = undefined;
    const minutes = this.state.settings?.autoClearAfterMinutes;
    if (minutes == null) return;
    const next = this.state.entries
      .flatMap((entry) =>
        entry.points.map(
          (point) =>
            new Date(point.submitted_at ?? entry.updated_at).getTime() +
            minutes * 60_000 -
            10 * 60_000,
        ),
      )
      .filter((time) => time > this.state.now)
      .sort((a, b) => a - b)[0];
    if (next === undefined) return;
    this.expiryTimer = this.timers.setTimeout(() => {
      this.expiryTimer = undefined;
      this.refreshNow();
    }, next - this.state.now);
  }
  dismiss(
    entry: StatusEntry,
    point: StatusPoint,
    pointIndex: number,
    removeKey: string,
    run: (args: DismissArgs) => Promise<void>,
    getSnapshot: () => Promise<Snapshot>,
  ): Promise<void> {
    if (
      this.disposed ||
      this.state.dismissing.has(removeKey) ||
      this.state.pendingRemoval.has(removeKey)
    )
      return Promise.resolve();
    this.state.dismissing.add(removeKey);
    return new Promise<void>((resolve) => {
      this.teardownWaiters.add(resolve);
      this.animations.enqueue({
        key: `dismiss:${removeKey}`,
        priority: 0,
        ready: () => true,
        run: (complete) => {
          this.state.pendingRemoval.add(removeKey);
          this.notify();
          void this.runDismissal(entry, point, pointIndex, removeKey, run, getSnapshot).finally(
            () => {
              this.teardownWaiters.delete(resolve);
              this.state.dismissing.delete(removeKey);
              if (!this.disposed) this.notify();
              complete();
              resolve();
            },
          );
        },
      });
      this.notify();
    });
  }
  private async runDismissal(
    entry: StatusEntry,
    point: StatusPoint,
    pointIndex: number,
    removeKey: string,
    run: (args: DismissArgs) => Promise<void>,
    getSnapshot: () => Promise<Snapshot>,
  ) {
    if (this.disposed) return;
    let exitTimer: ReturnType<typeof setTimeout> | undefined;
    const exit =
      this.motion?.dismiss(removeKey) ??
      new Promise<void>((resolve) => {
        exitTimer = this.timers.setTimeout(resolve, this.motionDuration(this.exitDuration));
        this.dismissalTimers.add(exitTimer);
      });
    let resolveDisposed!: () => void;
    const disposed = new Promise<void>((resolve) => {
      resolveDisposed = resolve;
      this.teardownWaiters.add(resolve);
    });
    try {
      await Promise.race([
        (async () => {
          const request = Promise.resolve().then(() => {
            if (this.disposed) return;
            return run({
              folderName: entry.folder_name,
              sessionId: entry.session_id,
              branch: entry.branch,
              updatedAt: entry.updated_at,
              pointId: point.id ?? null,
              pointIndex: point.id ? null : pointIndex,
            });
          });
          await Promise.all([request, exit]);
          if (this.disposed) return;
          // Always read after our request, even if a watcher supplied an earlier
          // snapshot while another dismissal was still in flight.
          const value = await getSnapshot();
          if (!this.disposed) this.acceptSnapshot(value);
        })(),
        disposed,
      ]);
    } catch (error) {
      if (!this.disposed) {
        this.state.pendingRemoval.delete(removeKey);
        this.motion?.restore(removeKey);
        this.pendingDismissalError = error instanceof Error ? error.message : String(error);
        this.setError(error);
      }
    } finally {
      if (exitTimer !== undefined) {
        this.timers.clearTimeout(exitTimer);
        this.dismissalTimers.delete(exitTimer);
      }
      this.teardownWaiters.delete(resolveDisposed);
    }
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.animations.dispose();
    this.motion?.dispose();
    if (this.motionTimer !== undefined) this.timers.clearTimeout(this.motionTimer);
    if (this.expiryTimer !== undefined) this.timers.clearTimeout(this.expiryTimer);
    this.dismissalTimers.forEach((timer) => this.timers.clearTimeout(timer));
    this.dismissalTimers.clear();
    this.teardownWaiters.forEach((resolve) => resolve());
    this.teardownWaiters.clear();
    this.listeners.clear();
  }
}
export function browserTimers(): TimerApi {
  return {
    setTimeout: window.setTimeout.bind(window),
    clearTimeout: window.clearTimeout.bind(window),
    now: () => Date.now(),
    reducedMotion: () => window.matchMedia('(prefers-reduced-motion: reduce)').matches,
  };
}
