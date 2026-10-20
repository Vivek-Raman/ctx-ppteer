import { listen } from '@tauri-apps/api/event';

export type UnlistenFn = () => void;

export function listenTauri<T>(
  eventName: string,
  onValue: (value: T) => void,
): Promise<UnlistenFn> {
  return listen<T>(eventName, (event) => onValue(event.payload));
}
