import type { Loader } from '../types';

export function loaderName(loader: Loader): string {
  return { vanilla: 'Vanilla', fabric: 'Fabric', forge: 'Forge' }[loader];
}
