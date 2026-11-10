import { Channel, invoke, isTauri } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type { AccountsSummary, AppSnapshot, DownloadProgress, FabricLoaderVersion, GameStatus, InstanceInput, InstalledMod, ModrinthPage, ModrinthVersion, RocketClientStatus, Settings, VersionCatalog } from '../types';
import type { ModpackDetails, ModpackFilters, ModpackProgress, MultiplayerAddon, SavedServer, ServerStatus } from '../types';

export const isDesktop = () => isTauri();

function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktop()) return Promise.reject(new Error('Open the desktop app with npm run desktop. Native storage is unavailable in a browser.'));
  return invoke<T>(command, args);
}

export const native = {
  accounts: () => call<AccountsSummary>('get_accounts'),
  createOfflineAccount: (name: string) => call<AccountsSummary>('create_offline_account', { name }),
  renameOfflineProfile: (id: string, name: string) => call<AccountsSummary>('rename_offline_profile', { id, name }),
  signIn: () => call<AccountsSummary>('microsoft_sign_in'),
  selectAccount: (id: string) => call<AccountsSummary>('select_account', { id }),
  createTestAccount: () => call<AccountsSummary>('create_local_test_account'),
  setOfflineMode: (offline: boolean) => call<AccountsSummary>('set_offline_mode', { offline }),
  signOut: (id: string) => call<AccountsSummary>('sign_out_account', { id }),
  checkGame: (id: string) => call<GameStatus>('check_game', { id }),
  installGame: (id: string) => call<void>('install_game', { id }),
  ignite: (id: string, server?: string) => call<void>('ignite_game', server ? { id, server } : { id }),
  abort: (id: string) => call<boolean>('abort_game', { instanceId: id }),
  instanceLog: (id: string, errors: boolean) => call<string | null>('read_instance_log', { id, errors }),
  savedServers: () => call<SavedServer[]>('saved_servers'),
  saveServer: (server: SavedServer) => call<SavedServer[]>('save_server', { server }),
  removeSavedServer: (id: string) => call<SavedServer[]>('remove_saved_server', { id }),
  pingServer: (address: string) => call<ServerStatus>('ping_server', { address }),
  multiplayerAddons: (instanceId: string) => call<MultiplayerAddon[]>('multiplayer_addons', { instanceId }),
  installMultiplayerAddon: (instanceId: string, feature: 'via' | 'hosting') =>
    call<string>('install_multiplayer_addon', { instanceId, feature }),
  downloadStatus: () => call<DownloadProgress | null>('download_status'),
  snapshot: () => call<AppSnapshot>('get_snapshot'),
  versions: (refresh = false) => call<VersionCatalog>('get_versions', { refresh }),
  fabricLoaders: (gameVersion: string) => call<FabricLoaderVersion[]>('get_fabric_loaders', { gameVersion }),
  fabricGameVersions: () => call<string[]>('get_fabric_game_versions'),
  forgeLoaders: (gameVersion: string) => call<FabricLoaderVersion[]>('get_forge_loaders', { gameVersion }),
  create: (input: InstanceInput) => call<AppSnapshot>('create_instance', { input }),
  update: (id: string, input: InstanceInput) => call<AppSnapshot>('update_instance', { id, input }),
  select: (id: string) => call<AppSnapshot>('select_instance', { id }),
  favorite: (id: string, favorite: boolean) => call<AppSnapshot>('set_instance_favorite', { id, favorite }),
  remove: (id: string) => call<AppSnapshot>('remove_instance', { id }),
  duplicate: (id: string) => call<AppSnapshot>('duplicate_instance', { id }),
  settings: (settings: Settings) => call<AppSnapshot>('save_settings', { settings }),
  mods: (id: string) => call<InstalledMod[]>('list_installed_mods', { id }),
  setModEnabled: (instanceId: string, fileName: string, enabled: boolean) => call<void>('set_instance_mod_enabled', { instanceId, fileName, enabled }),
  openModsFolder: (instanceId: string) => call<void>('open_instance_mods_folder', { instanceId }),
  openGameFolder: (instanceId: string) => call<void>('open_instance_folder', { instanceId }),
  rocketClientStatus: (instanceId: string) => call<RocketClientStatus>('rocket_client_status', { instanceId }),
  installRocketClient: (instanceId: string) => call<void>('install_rocket_client', { instanceId }),
  searchMods: (instanceId: string, query: string, offset: number, sort?: string) => call<ModrinthPage>('search_modrinth', { instanceId, query, offset, sort }),
  searchModpacks: (query: string, offset: number, sort: string, filters: ModpackFilters) =>
    call<ModrinthPage>('search_modpacks', { query, offset, sort, filters }),
  modpackDetails: (projectId: string) => call<ModpackDetails>('modpack_details', { projectId }),
  installModpack: (projectId: string, versionId: string, game: string, loader: string, onProgress: (value: ModpackProgress) => void) => {
    const progress = new Channel<ModpackProgress>();
    progress.onmessage = onProgress;
    return call<AppSnapshot>('install_modpack', { projectId, versionId, game, loader, progress });
  },
  modVersions: (instanceId: string, projectId: string) => call<ModrinthVersion[]>('modrinth_versions', { instanceId, projectId }),
  installMod: (instanceId: string, projectId: string, versionId?: string) => call<string>('install_modrinth', { instanceId, projectId, versionId }),
  installRocketPack: (version: string, onProgress: (files: [number, number]) => void, input?: InstanceInput) => {
    const progress = new Channel<[number, number]>();
    progress.onmessage = onProgress;
    return call<AppSnapshot>('install_rocket_pack', { version, progress, input });
  },
  checkUpdate: () => call<string | null>('check_launcher_update'),
  installUpdate: (releaseVersion: string, onProgress: (percent: number) => void) => {
    const progress = new Channel<number>();
    progress.onmessage = onProgress;
    return call<void>('install_launcher_update', { releaseVersion, progress });
  },
  logs: () => call<string>('read_logs'),
  chooseDirectory: () => open({ directory: true, multiple: false, title: 'Choose an instance parent directory' }),
  chooseJava: () => open({ multiple: false, title: 'Choose a Java executable', filters: [{ name: 'Java executable', extensions: ['exe'] }] }),
};

export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === 'object' && error !== null && 'message' in error) return String(error.message);
  return typeof error === 'string' ? error : 'Something went wrong. Please try again.';
}
