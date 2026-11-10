export type Page = 'launchpad' | 'instances' | 'mods' | 'multiplayer' | 'downloads' | 'settings';
export interface SavedServer {
  id: string;
  name: string;
  address: string;
  instanceId: string | null;
  favorite: boolean;
  lastJoined: number | null;
}
export interface ServerStatus {
  version: string;
  protocol: number;
  players: number;
  maxPlayers: number;
  latencyMs: number;
}
export interface MultiplayerAddon { id: string; filename: string; enabled: boolean }
export type Loader = 'vanilla' | 'fabric' | 'forge';
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
  syncServers: boolean;
  syncKeybinds: boolean;
  syncRocketClient: boolean;
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
  rocketPack?: boolean;
  favorite?: boolean;
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
export interface InstalledMod { fileName: string; sizeBytes: number; enabled: boolean }
export interface RocketClientStatus {
  state: 'unsupported' | 'blocked' | 'available' | 'ready' | 'disabled' | 'upgrade';
  message: string;
  needsDownload: boolean;
}
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
  categories?: string[];
  versions?: string[];
  dateModified?: string | null;
  gallery?: string[];
}
export interface ModpackFilters { game: string; loader: string; category: string; side: string; collection: string }
export interface ModpackProgress { phase: string; completed: number; total: number }
export interface ModpackVersion extends Omit<ModrinthVersion, 'fileName'> {
  datePublished: string;
  gameVersions: string[];
  loaders: string[];
}
export interface ModpackDetails {
  body: string;
  gallery: { url: string; title: string | null; description: string | null; featured: boolean }[];
  versions: ModpackVersion[];
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
