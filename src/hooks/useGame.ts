import { useCallback, useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { errorMessage, isDesktop, native } from '../services/native';
import type { AppSnapshot, DownloadProgress, GameStatus, Instance, Toast } from '../types';

export function useGame(snapshot: AppSnapshot | null, notify: (toast: Toast) => void, accountRevision?: object | null) {
  const [statuses, setStatuses] = useState<Record<string, GameStatus>>({});
  const [download, setDownload] = useState<DownloadProgress | null>(null);
  const [working, setWorking] = useState(false);
  const operation = useRef(false);
  const revisions = useRef(new Map<string, number>());
  const publish = useCallback((status: GameStatus) => {
    revisions.current.set(status.instanceId, (revisions.current.get(status.instanceId) ?? 0) + 1);
    setStatuses((old) => ({ ...old, [status.instanceId]: status }));
  }, []);
  const check = useCallback(async (id: string) => {
    const version = (revisions.current.get(id) ?? 0) + 1;
    revisions.current.set(id, version);
    try {
      const status = await native.checkGame(id);
      if (revisions.current.get(id) === version) publish(status);
    } catch (error) {
      if (revisions.current.get(id) === version) publish({ instanceId: id, readiness: 'error', message: errorMessage(error), exitCode: null });
    }
  }, [publish]);

  useEffect(() => {
    if (!isDesktop()) return;
    let stopped = false;
    let received = false;
    const offStatus = listen<GameStatus>('game-status', ({ payload }) => {
      if (!stopped) publish(payload);
    });
    const offDownload = listen<DownloadProgress>('game-download', ({ payload }) => {
      if (stopped) return;
      received = true;
      setDownload(payload);
    });
    void offDownload.then(() => native.downloadStatus()).then((value) => {
      if (!stopped && !received) setDownload(value);
    }).catch(() => {});
    void offStatus.catch(() => {});
    return () => {
      stopped = true;
      void offStatus.then((off) => off()).catch(() => {});
      void offDownload.then((off) => off()).catch(() => {});
    };
  }, [publish]);

  // A rename/selection does not re-hash the shared game cache. Version and Java edits do.
  const profiles = JSON.stringify(snapshot?.instances.map((instance) =>
    [instance.id, instance.version, instance.loader, instance.loaderVersion, instance.javaPath ?? snapshot.settings.javaPath]) ?? []);
  useEffect(() => {
    let stopped = false;
    const entries = JSON.parse(profiles) as [string][];
    setStatuses((previous) => Object.fromEntries(entries.map<[string, GameStatus]>(([id]) => [id, {
      instanceId: id, readiness: 'checking', message: 'Checking local files…', exitCode: null,
    } satisfies GameStatus]).concat(Object.entries(previous).filter(([id, status]) =>
      entries.some(([entry]) => id === entry) && (status.readiness === 'running' || status.readiness === 'launching'),
    ))));
    void (async () => {
      for (const [id] of entries) {
        if (stopped) break;
        await check(id);
      }
    })();
    return () => {
      stopped = true;
      for (const [id] of entries) revisions.current.set(id, (revisions.current.get(id) ?? 0) + 1);
    };
  }, [profiles, check, accountRevision]);

  async function run(instance: Instance, action: 'install' | 'launch', server?: string) {
    if (operation.current) return false;
    operation.current = true;
    setWorking(true);
    if (action === 'install') publish({ instanceId: instance.id, readiness: 'downloading', message: 'Preparing installation…', exitCode: null });
    try {
      if (action === 'install') {
        await native.installGame(instance.id);
        await check(instance.id);
        notify({ kind: 'success', message: 'Minecraft files verified.' });
      } else {
        await native.ignite(instance.id, server);
      }
      return true;
    } catch (error) {
      const message = errorMessage(error);
      const missing = typeof error === 'object' && error !== null && 'code' in error && error.code === 'MISSING_FILES';
      publish({ instanceId: instance.id, readiness: missing ? 'missing_files' : 'error', message, exitCode: null });
      notify({ kind: 'error', message });
      return false;
    } finally {
      if (action === 'install') {
        try { setDownload(await native.downloadStatus()); } catch { /* Events also carry the latest state. */ }
      }
      operation.current = false;
      setWorking(false);
    }
  }

  function status(instance: Instance): GameStatus {
    if (download?.instanceId === instance.id && download.version === instance.version &&
        (download.stage === 'downloading' || download.stage === 'installing')) {
      return { instanceId: instance.id, readiness: download.stage, message: download.current, exitCode: null };
    }
    return statuses[instance.id] ?? { instanceId: instance.id, readiness: instance.readiness, message: 'Checking local files…', exitCode: null };
  }

  return { status, download, working, run, check };
}
