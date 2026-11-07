export type Page = 'launchpad' | 'instances' | 'mods' | 'downloads' | 'settings';
export type Loader = 'vanilla' | 'fabric';
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
  loader: Loader;
  loaderVersion: string | null;
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
export interface AccountsSummary {
  profiles: { id: string; name: string; type: 'microsoft' | 'local_test' | 'offline' }[];
  activeId: string | null;
  offline: boolean;
}
export interface InstanceInput {
  name: string;
  version: string;
  loader: Loader;
  loaderVersion: string | null;
  ramMb: number;
  javaPath: string | null;
  parentDirectory: string | null;
}
export interface FabricLoaderVersion { version: string; stable: boolean }
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
export interface ModrinthProject {
  projectId: string;
  title: string;
  description: string;
  author: string;
  downloads: number;
  iconUrl?: string | null;
  featuredGallery?: string | null;
  displayCategories?: string[];
}
export interface ModrinthPage {
  hits: ModrinthProject[];
  totalHits: number;
}
export interface ModrinthVersion {
  id: string;
  versionNumber: string;
  versionType: string;
  fileName: string;
  requiredDependencies: string[];
}
