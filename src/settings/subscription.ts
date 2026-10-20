import type { Settings } from '../shared/types';
import { startEventReadLifecycle, type EventReadLifecycle } from '../shared/event-lifecycle';
import { listenTauri } from '../shared/tauri-events';

export function subscribeSettings(
  onSettings: (settings: Settings) => void,
  onError: (error: unknown) => void,
  readSettings?: () => Promise<Settings>,
): EventReadLifecycle<Settings> {
  return startEventReadLifecycle({
    listen: (onValue) => listenTauri('settings-update', onValue),
    read: readSettings,
    onValue: onSettings,
    onError,
  });
}
