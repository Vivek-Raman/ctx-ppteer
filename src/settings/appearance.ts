import type { Settings } from '../shared/types';
import { command } from '../shared/commands';

export async function saveAppearance(
  settings: Pick<Settings, 'theme' | 'primaryColor' | 'textScale'>,
) {
  await command('set_appearance_settings', {
    theme: settings.theme,
    primaryColor: settings.primaryColor,
    textScale: Number(settings.textScale),
  });
}

export async function savePinned(pinned: boolean) {
  await command('set_pinned', { pinned });
}
