import type { ViewerLayout, ViewerMotion } from './controller';
import { projectNames, viewerRows } from './rows';

const easing = 'cubic-bezier(0.22, 1, 0.36, 1)';

// Owns only content animations. Each promise settles on the browser's actual
// animation completion, including cancellation during teardown.
export class RowMotion implements ViewerMotion {
  private rows = new Map<string, HTMLElement>();
  private fresh = new Set<string>();
  private faded = new Set<string>();
  private animations = new Set<Animation>();
  private dismissals = new Map<string, Animation>();
  private dismissingKeys = new Set<string>();
  private disposed = false;

  constructor(
    private readonly tick: () => Promise<void>,
    private readonly duration: (ms: number) => number,
  ) {}

  row = (node: HTMLElement, key: string) => {
    this.rows.set(key, node);
    this.fresh.add(key);
    if (this.duration(220)) (node.firstElementChild as HTMLElement).style.opacity = '0';
    return {
      destroy: () => {
        this.restore(key);
        this.rows.delete(key);
        this.fresh.delete(key);
        this.faded.delete(key);
      },
    };
  };

  private async animate(node: HTMLElement, frames: Keyframe[], duration: number, phase: string) {
    if (this.disposed || !duration) return;
    const animation = node.animate(frames, { duration, easing, fill: 'both' });
    animation.id = phase;
    this.animations.add(animation);
    try {
      await animation.finished;
    } catch {
      // A removed row or a disposed viewer cancels its animations.
    } finally {
      animation.cancel();
      this.animations.delete(animation);
    }
  }

  async exit(layout: ViewerLayout) {
    await this.tick();
    if (this.disposed) return;
    const next = new Set(
      viewerRows(
        layout.entries,
        projectNames(layout.entries, layout.settings?.projectSortOrder ?? 'project-directory-asc'),
      ).map(({ key }) => key),
    );
    const exits: Promise<void>[] = [];
    for (const [key, node] of this.rows) {
      if (next.has(key)) {
        if (this.faded.delete(key)) node.style.opacity = '';
      } else if (!this.faded.has(key)) {
        this.faded.add(key);
        // Dismissed points already completed their own exit. Preserve their
        // empty space, without making the text visible for another fade.
        const alreadyHidden = node.querySelector('.removing') || this.fresh.has(key);
        exits.push(
          (async () => {
            if (!alreadyHidden)
              await this.animate(
                node,
                [{ opacity: 1 }, { opacity: 0 }],
                this.duration(180),
                'row-exit',
              );
            if (!this.disposed) node.style.opacity = '0';
          })(),
        );
      }
    }
    await Promise.all(exits);
  }

  async move(commit: () => void) {
    const before = new Map(
      [...this.rows].map(([key, node]) => [key, node.getBoundingClientRect()]),
    );
    commit();
    await this.tick();
    if (this.disposed) return;
    const moves: Promise<void>[] = [];
    for (const [key, node] of this.rows) {
      const from = before.get(key);
      if (!from || this.fresh.has(key)) continue;
      const to = node.getBoundingClientRect();
      const x = from.left - to.left;
      const y = from.top - to.top;
      if (Math.abs(x) > 0.5 || Math.abs(y) > 0.5) {
        moves.push(
          this.animate(
            node,
            [{ transform: `translate(${x}px, ${y}px)` }, { transform: 'translate(0, 0)' }],
            this.duration(300),
            'row-move',
          ),
        );
      }
    }
    await Promise.all(moves);
  }

  async enter() {
    await this.tick();
    if (this.disposed) return;
    await Promise.all(
      [...this.fresh].map(async (key) => {
        const node = this.rows.get(key)?.firstElementChild as HTMLElement | undefined;
        if (!node) return;
        await this.animate(
          node,
          [
            { opacity: 0, transform: 'translateY(18px)' },
            { opacity: 1, transform: 'translateY(0)' },
          ],
          this.duration(220),
          'row-enter',
        );
        if (!this.disposed) node.style.opacity = '';
        this.fresh.delete(key);
      }),
    );
  }

  async dismiss(key: string) {
    this.dismissingKeys.add(key);
    await this.tick();
    if (this.disposed || !this.dismissingKeys.has(key)) return;
    const node = this.rows.get(key)?.querySelector<HTMLElement>('.status-point');
    if (!node) return;
    const duration = this.duration(320);
    if (!duration) {
      node.style.opacity = '0';
      return;
    }
    const animation = node.animate(
      [
        { opacity: getComputedStyle(node).opacity, transform: 'translateX(0)' },
        { opacity: 0, transform: 'translateX(18px)' },
      ],
      { duration, easing: 'cubic-bezier(0.4, 0, 1, 1)', fill: 'forwards' },
    );
    animation.id = 'row-dismiss';
    this.animations.add(animation);
    this.dismissals.set(key, animation);
    await animation.finished.catch(() => undefined);
  }

  restore(key: string) {
    this.dismissingKeys.delete(key);
    const animation = this.dismissals.get(key);
    if (animation) {
      animation.cancel();
      this.animations.delete(animation);
      this.dismissals.delete(key);
    }
    const node = this.rows.get(key)?.querySelector<HTMLElement>('.status-point');
    if (node) node.style.opacity = '';
  }

  dispose() {
    this.disposed = true;
    this.animations.forEach((animation) => animation.cancel());
    this.animations.clear();
    this.dismissals.clear();
    this.dismissingKeys.clear();
  }
}
