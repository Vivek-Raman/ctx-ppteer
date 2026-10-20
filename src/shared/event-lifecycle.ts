export type UnlistenFn = () => void;

export type EventReadLifecycle<T> = { dispose(): void; ready: Promise<void> };

export type EventReadLifecycleOptions<T> = {
  listen: (onValue: (value: T) => void, onError: (error: unknown) => void) => Promise<UnlistenFn>;
  read?: () => Promise<T>;
  onValue: (value: T) => void;
  onError: (error: unknown) => void;
};

export function startEventReadLifecycle<T>(
  options: EventReadLifecycleOptions<T>,
): EventReadLifecycle<T> {
  let disposed = false;
  let generation = 0;
  let unlisten: UnlistenFn | undefined;

  const ready = options
    .listen(
      (value) => {
        if (disposed) return;
        generation += 1;
        options.onValue(value);
      },
      (error) => {
        if (!disposed) options.onError(error);
      },
    )
    .then((stop) => {
      if (disposed) {
        void stop();
        return;
      }
      unlisten = stop;
      if (!options.read) return;
      const readGeneration = generation;
      return options.read().then((value) => {
        if (!disposed && readGeneration === generation) options.onValue(value);
      });
    })
    .catch((error) => {
      if (!disposed) options.onError(error);
    });

  return {
    ready,
    dispose() {
      disposed = true;
      unlisten?.();
    },
  };
}
