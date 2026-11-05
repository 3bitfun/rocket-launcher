export type Page = 'launchpad' | 'instances' | 'mods' | 'downloads' | 'settings';
export type Theme = 'dark' | 'light' | 'system';
export type Readiness = 'checking' | 'requires_download' | 'missing_files' | 'downloading' | 'installing' | 'java_required' | 'authentication_required' | 'ready' | 'launching' | 'running' | 'error';
export interface GameStatus { instanceId: string; readiness: Readiness; message: string; exitCode: number | null }
export interface DownloadProgress {
  instanceId: string; version: string; stage: 'downloading' | 'installing' | 'complete' | 'error';
  current: string; transferredBytes: number; verifiedBytes: number; totalBytes: number | null; bytesPerSecond: number; error: string | null;
}
export interface Settings {
  accentColor: string;
  theme: Theme;
  animations: boolean;
  reducedMotion: 'system' | 'always';
  defaultRamMb: number;
  javaPath: string | null;
  minecraftDirectory: string;
  showSnapshots: boolean;
  requestTimeoutSeconds: number;
}
export interface Instance {
  id: string;
  name: string;
  version: string;
  versionType: 'release' | 'snapshot';
  loader: 'vanilla';
  gameDirectory: string;
  ramMb: number;
  javaPath: string | null;
  createdAt: number;
  updatedAt: number;
  lastSelectedAt: number;
  readiness: Readiness;
}
export interface AppSnapshot {
  appVersion: string;
  settings: Settings;
  instances: Instance[];
  activeInstanceId: string | null;
  paths: { data: string; cache: string; logs: string; config: string };
}
export interface InstanceInput {
  name: string;
  version: string;
  ramMb: number;
  javaPath: string | null;
  parentDirectory: string | null;
}
export interface MinecraftVersion {
  id: string;
  type: 'release' | 'snapshot';
  url: string;
  releaseTime: string;
  sha1: string;
}
export interface VersionCatalog {
  latest: { release: string; snapshot: string };
  versions: MinecraftVersion[];
  fetchedAt: number;
  source: 'network' | 'cache';
  warning: string | null;
}
export interface InstalledMod { fileName: string; sizeBytes: number }
export interface Toast { message: string; kind: 'success' | 'error' }
