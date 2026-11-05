import { useCallback, useEffect, useRef, useState } from 'react';
import { errorMessage, native } from '../services/native';
import type { AppSnapshot, Toast, VersionCatalog } from '../types';

export function useLauncher() {
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [startupError, setStartupError] = useState<string | null>(null);
  const [catalog, setCatalog] = useState<VersionCatalog | null>(null);
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const [fetching, setFetching] = useState(false);
  const [busy, setBusy] = useState(false);
  const [toast, setToast] = useState<Toast | null>(null);
  const inFlight = useRef(false);
  const mutation = useRef(false);

  const refreshVersions = useCallback(async (force = true) => {
    if (inFlight.current) return;
    inFlight.current = true;
    setFetching(true);
    setCatalogError(null);
    try { setCatalog(await native.versions(force)); }
    catch (error) { setCatalogError(errorMessage(error)); }
    finally { inFlight.current = false; setFetching(false); }
  }, []);

  const load = useCallback(async () => {
    setStartupError(null);
    try { setSnapshot(await native.snapshot()); void refreshVersions(false); }
    catch (error) { setStartupError(errorMessage(error)); }
  }, [refreshVersions]);

  useEffect(() => { void load(); }, [load]);
  useEffect(() => {
    if (!toast || toast.kind === 'error') return;
    const timer = window.setTimeout(() => setToast(null), 4500);
    return () => window.clearTimeout(timer);
  }, [toast]);

  const mutate = useCallback(async (action: () => Promise<AppSnapshot>, message: string) => {
    if (mutation.current) return false;
    mutation.current = true;
    setBusy(true);
    setToast(null);
    try { setSnapshot(await action()); setToast({ kind: 'success', message }); return true; }
    catch (error) { setToast({ kind: 'error', message: errorMessage(error) }); return false; }
    finally { mutation.current = false; setBusy(false); }
  }, []);

  return { snapshot, startupError, catalog, catalogError, fetching, busy, toast, setToast, refreshVersions, load, mutate };
}

export type Launcher = ReturnType<typeof useLauncher>;
