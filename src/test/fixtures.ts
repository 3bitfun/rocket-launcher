import type { AppSnapshot, VersionCatalog } from '../types';

// Unit-test fixtures only. Production UI always uses the Rust command bridge.
export const snapshot: AppSnapshot = {
  appVersion: '0.1.0', activeInstanceId: null, instances: [],
  paths: { data: 'C:\\test', cache: 'C:\\test\\cache', logs: 'C:\\test\\logs', config: 'C:\\test\\launcher.json' },
  settings: { accentColor: '#FF704D', theme: 'dark', animations: true, reducedMotion: 'system', defaultRamMb: 4096, javaPath: null, minecraftDirectory: 'C:\\test\\instances', showSnapshots: false, requestTimeoutSeconds: 20, syncServers: true, syncKeybinds: true },
};
export const catalog: VersionCatalog = {
  latest: { release: '1.21.1', snapshot: '24w33a' }, fetchedAt: 1723719600, source: 'network', warning: null,
  versions: [
    { id: '1.21.1', type: 'release', url: 'https://piston-meta.mojang.com/test.json', sha1: '0'.repeat(40), releaseTime: '2024-08-08T12:00:00Z' },
    { id: '24w33a', type: 'snapshot', url: 'https://piston-meta.mojang.com/test.json', sha1: '1'.repeat(40), releaseTime: '2024-08-15T12:00:00Z' },
  ],
};
