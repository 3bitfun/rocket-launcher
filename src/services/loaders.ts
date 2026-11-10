import type { Instance, Loader } from '../types';

export function loaderName(loader: Loader): string {
  return { vanilla: 'Vanilla', fabric: 'Fabric', forge: 'Forge' }[loader];
}
export const recommendedRocketVersion = '1.21.11';

export function rocketInstancesByVersion(instances: Instance[]) {
  const versions = new Map<string, Instance>();
  for (const instance of instances) {
    if (!instance.rocketPack) continue;
    const current = versions.get(instance.version);
    if (!current || instance.lastSelectedAt > current.lastSelectedAt) versions.set(instance.version, instance);
  }
  return versions;
}

export function rocketInstallLabel(instance?: Instance) {
  if (!instance) return 'Not installed';
  switch (instance.readiness) {
    case 'checking': return 'Checking files…';
    case 'requires_download': return 'Minecraft download needed';
    case 'missing_files': return 'Needs repair';
    case 'downloading':
    case 'installing': return 'Installing…';
    case 'error': return 'Check files';
    default: return 'Installed';
  }
}
