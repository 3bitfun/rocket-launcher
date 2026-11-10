import type { Loader } from '../types';

export function loaderName(loader: Loader): string {
  return { vanilla: 'Vanilla', fabric: 'Fabric', forge: 'Forge' }[loader];
}
export const recommendedRocketVersion = '1.21.11';
