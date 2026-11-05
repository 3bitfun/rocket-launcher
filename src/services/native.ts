import { invoke, isTauri } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type { AppSnapshot, DownloadProgress, GameStatus, InstanceInput, InstalledMod, Settings, VersionCatalog } from '../types';

export const isDesktop = () => isTauri();

function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktop()) return Promise.reject(new Error('Open the desktop app with npm run desktop. Native storage is unavailable in a browser.'));
  return invoke<T>(command, args);
}

export const native = {
  checkGame: (id: string) => call<GameStatus>('check_game', { id }),
  installGame: (id: string) => call<void>('install_game', { id }),
  ignite: (id: string) => call<void>('ignite_game', { id }),
  downloadStatus: () => call<DownloadProgress | null>('download_status'),
  snapshot: () => call<AppSnapshot>('get_snapshot'),
  versions: (refresh = false) => call<VersionCatalog>('get_versions', { refresh }),
  create: (input: InstanceInput) => call<AppSnapshot>('create_instance', { input }),
  update: (id: string, input: InstanceInput) => call<AppSnapshot>('update_instance', { id, input }),
  select: (id: string) => call<AppSnapshot>('select_instance', { id }),
  remove: (id: string) => call<AppSnapshot>('remove_instance', { id }),
  duplicate: (id: string) => call<AppSnapshot>('duplicate_instance', { id }),
  settings: (settings: Settings) => call<AppSnapshot>('save_settings', { settings }),
  mods: (id: string) => call<InstalledMod[]>('list_installed_mods', { id }),
  logs: () => call<string>('read_logs'),
  chooseDirectory: () => open({ directory: true, multiple: false, title: 'Choose an instance parent directory' }),
  chooseJava: () => open({ multiple: false, title: 'Choose a Java executable', filters: [{ name: 'Java executable', extensions: ['exe'] }] }),
};

export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === 'object' && error !== null && 'message' in error) return String(error.message);
  return typeof error === 'string' ? error : 'Something went wrong. Please try again.';
}
