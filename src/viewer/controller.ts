import type { Settings, Snapshot, StatusEntry, StatusPoint } from '../shared/types';

export type ViewerState = {
  revision: number;
  entries: StatusEntry[];
  settings: Settings | null;
  pendingSnapshot: Snapshot | null;
  pendingSettings: Settings | null;
  selectedText: boolean;
  pendingRemoval: string;
  dismissing: string;
  statusError: string;
  now: number;
};
export type TimerApi = {
  setTimeout(handler: () => void, delay: number): ReturnType<typeof setTimeout>;
  clearTimeout(id: ReturnType<typeof setTimeout>): void;
  now(): number;
  reducedMotion(): boolean;
};
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
  pendingRemoval: '',
  dismissing: '',
  statusError: '',
  now: Date.now(),
});

export class ViewerController {
  readonly state: ViewerState = initialState();
  private motionTimer: ReturnType<typeof setTimeout> | undefined;
  private expiryTimer: ReturnType<typeof setTimeout> | undefined;
  private disposed = false;
  private readonly exitDuration = 320;
  private readonly timers: TimerApi;
  private readonly listeners = new Set<Listener>();
  private readonly teardownWaiters = new Set<() => void>();
  private readonly dismissalTimers = new Set<ReturnType<typeof setTimeout>>();
  private notifying = false;
  private notifyAgain = false;
  constructor(timers: TimerApi) {
    this.timers = timers;
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
    if (this.motionTimer !== undefined) {
      this.state.pendingSettings = value;
      this.notify();
      return;
    }
    this.commitSettings(value);
  }
  private commitSettings(value: Settings) {
    if (this.disposed) return;
    const layoutChanged =
      this.state.settings &&
      (this.state.settings.projectSortOrder !== value.projectSortOrder ||
        this.state.settings.textScale !== value.textScale);
    this.state.settings = { ...value };
    this.state.pendingSettings = null;
    this.scheduleExpiry();
    this.notify();
    if (layoutChanged) this.settleMotion();
  }
  acceptSnapshot(value: Snapshot) {
    if (
      this.disposed ||
      value.revision <= this.state.revision ||
      (this.state.pendingSnapshot && value.revision < this.state.pendingSnapshot.revision)
    )
      return;
    if (this.state.selectedText || this.state.pendingRemoval || this.motionTimer !== undefined) {
      this.state.pendingSnapshot = value;
      this.notify();
      return;
    }
    this.commitSnapshot(value);
  }
  private commitSnapshot(value: Snapshot) {
    if (this.disposed) return;
    this.state.revision = value.revision;
    this.state.pendingSnapshot = null;
    this.state.entries = value.document.entries;
    this.state.statusError = value.error ?? '';
    this.scheduleExpiry();
    this.notify();
    this.settleMotion();
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
  private flushPending() {
    if (
      this.disposed ||
      this.state.selectedText ||
      this.state.pendingRemoval ||
      this.motionTimer !== undefined
    )
      return;
    const settings = this.state.pendingSettings;
    const snapshot = this.state.pendingSnapshot;
    if (settings) this.commitSettings(settings);
    if (snapshot && !this.state.selectedText && !this.state.pendingRemoval)
      this.commitSnapshot(snapshot);
  }
  settleMotion() {
    if (this.disposed) return;
    if (this.motionTimer !== undefined) this.timers.clearTimeout(this.motionTimer);
    this.motionTimer = this.timers.setTimeout(() => {
      this.motionTimer = undefined;
      this.flushPending();
      this.notify();
    }, this.motionDuration(600));
    this.notify();
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
  async dismiss(
    entry: StatusEntry,
    point: StatusPoint,
    pointIndex: number,
    removeKey: string,
    run: (args: DismissArgs) => Promise<void>,
    getSnapshot: () => Promise<Snapshot>,
  ) {
    if (this.disposed || this.state.pendingRemoval) return;
    this.state.dismissing = `${entry.folder_name}:${entry.session_id ?? ''}:${entry.updated_at}:${pointIndex}`;
    this.state.pendingRemoval = removeKey;
    this.notify();
    let exitTimer: ReturnType<typeof setTimeout> | undefined;
    const exit = new Promise<void>((resolve) => {
      exitTimer = this.timers.setTimeout(resolve, this.motionDuration(this.exitDuration));
      this.dismissalTimers.add(exitTimer);
    });
    let resolveDisposed!: () => void;
    const disposed = new Promise<void>((resolve) => {
      resolveDisposed = resolve;
      this.teardownWaiters.add(resolve);
    });
    try {
      const request = run({
        folderName: entry.folder_name,
        sessionId: entry.session_id,
        branch: entry.branch,
        updatedAt: entry.updated_at,
        pointId: point.id ?? null,
        pointIndex: point.id ? null : pointIndex,
      });
      await Promise.race([Promise.all([request, exit]), disposed]);
      if (this.disposed) return;
      this.state.pendingRemoval = '';
      this.notify();
      this.acceptSnapshot(this.state.pendingSnapshot ?? (await getSnapshot()));
    } catch (error) {
      if (!this.disposed) {
        this.state.pendingRemoval = '';
        this.state.dismissing = '';
        this.setError(error);
        if (this.motionTimer === undefined) this.flushPending();
      }
    } finally {
      if (exitTimer !== undefined) {
        this.timers.clearTimeout(exitTimer);
        this.dismissalTimers.delete(exitTimer);
      }
      this.teardownWaiters.delete(resolveDisposed);
      if (!this.disposed) {
        this.state.dismissing = '';
        this.notify();
      }
    }
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
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
