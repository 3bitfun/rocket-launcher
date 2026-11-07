import type { Loader } from '../types';

export function loaderName(loader: Loader): string {
  return loader === 'fabric' ? 'Fabric' : 'Vanilla';
}
