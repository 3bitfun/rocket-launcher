import { useEffect, useState } from 'react';
import { Download, FileArchive, FolderOpen, Puzzle, RefreshCw, Search } from 'lucide-react';
import type { AppSnapshot, InstalledMod, VersionCatalog } from '../../types';
import { native, errorMessage } from '../../services/native';
import { EmptyState, Spinner } from '../../components/ui/Primitives';
import { ModrinthBrowser } from './ModrinthBrowser';

const recommendedVersion = '1.21.11';

export function Mods({ snapshot, catalog, offline, busy, initialInstanceId, onPlay, onInstallPack, onModpackInstalled }: { snapshot: AppSnapshot; catalog: VersionCatalog | null; offline: boolean; busy: boolean; initialInstanceId?: string; onPlay: () => void; onInstallPack: (version: string, progress: (files: [number, number]) => void) => Promise<boolean>; onModpackInstalled?: () => void }) {
  const [view, setView] = useState<'browse' | 'installed' | 'pack'>(initialInstanceId ? 'installed' : snapshot.instances.length ? 'browse' : 'pack');
  const [id, setId] = useState(initialInstanceId ?? snapshot.activeInstanceId ?? '');
  const [mods, setMods] = useState<InstalledMod[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [revision, setRevision] = useState(0);
  const [filter, setFilter] = useState('');
  const [changingMod, setChangingMod] = useState<string | null>(null);
  const [selectedMods, setSelectedMods] = useState<Set<string>>(new Set());
  const [installingPack, setInstallingPack] = useState(false);
  const [packInstalled, setPackInstalled] = useState(false);
  const [packProgress, setPackProgress] = useState<[number, number] | null>(null);
  const [packVersion, setPackVersion] = useState(recommendedVersion);
  const [fabricVersions, setFabricVersions] = useState<string[]>([]);
  const [packListError, setPackListError] = useState('');
  const packVersions = (catalog?.versions ?? []).filter((version) => version.type === 'release' && version.id !== recommendedVersion && fabricVersions.includes(version.id));
  const instance = snapshot.instances.find((item) => item.id === id);
  const visibleMods = mods.filter((mod) => mod.fileName.toLowerCase().includes(filter.trim().toLowerCase()));
  const running = instance?.readiness === 'running' || instance?.readiness === 'launching';
  useEffect(() => {
    if (offline || view !== 'pack') return;
    let active = true;
    setPackListError('');
    void native.fabricGameVersions()
      .then((versions) => { if (active) setFabricVersions(versions); })
      .catch((reason) => { if (active) setPackListError(errorMessage(reason)); });
    return () => { active = false; };
  }, [offline, view]);
  useEffect(() => { setId(initialInstanceId ?? snapshot.activeInstanceId ?? ''); }, [initialInstanceId, snapshot.activeInstanceId]);
  useEffect(() => { setSelectedMods(new Set()); setError(null); }, [id]);
  useEffect(() => {
    let cancelled = false;
    if (!id || view !== 'installed') return;
    setLoading(true); setMods([]);
    native.mods(id).then((value) => { if (!cancelled) setMods(value); }).catch((reason) => { if (!cancelled) setError(errorMessage(reason)); }).finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [id, revision, view]);

  async function installPack() {
    setInstallingPack(true);
    setPackProgress(null);
    try { setPackInstalled(await onInstallPack(packVersion, setPackProgress)); }
    finally { setInstallingPack(false); }
  }

  async function toggleMod(mod: InstalledMod) {
    setChangingMod(mod.fileName);
    setError(null);
    try {
      await native.setModEnabled(id, mod.fileName, !mod.enabled);
      setRevision((value) => value + 1);
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setChangingMod(null);
    }
  }

  async function changeSelected(enabled: boolean) {
    const pending = mods.filter((mod) => selectedMods.has(mod.fileName) && mod.enabled !== enabled);
    if (!pending.length) return;
    setChangingMod('bulk');
    setError(null);
    let completed = 0;
    try {
      for (const mod of pending) {
        await native.setModEnabled(id, mod.fileName, enabled);
        completed++;
      }
      setSelectedMods(new Set());
    } catch (reason) {
      setError(`Changed ${completed} of ${pending.length} mods. ${errorMessage(reason)}`);
    } finally {
      setRevision((value) => value + 1);
      setChangingMod(null);
    }
  }

  function selectMod(fileName: string, checked: boolean) {
    setSelectedMods((previous) => {
      const next = new Set(previous);
      if (checked) next.add(fileName);
      else next.delete(fileName);
      return next;
    });
  }

  async function openFolder() {
    setError(null);
    try { await native.openModsFolder(id); }
    catch (reason) { setError(errorMessage(reason)); }
  }

  return <>
    <div className="mod-workspace-bar">
      <div className="segmented" role="group" aria-label="Mods view">
        <button type="button" aria-pressed={view === 'browse'} className={view === 'browse' ? 'active' : ''} onClick={() => setView('browse')}>Browse mods</button>
        <button type="button" aria-pressed={view === 'installed'} className={view === 'installed' ? 'active' : ''} onClick={() => setView('installed')}>Installed mods</button>
        <button type="button" aria-pressed={view === 'pack'} className={view === 'pack' ? 'active' : ''} onClick={() => setView('pack')}>Rocket Pack</button>
      </div>
      {view !== 'pack' && <label className="instance-target"><span>Target instance</span><select value={id} onChange={(e) => setId(e.target.value)} disabled={!snapshot.instances.length || busy || changingMod !== null}>
        {!id && <option value="">Select an instance</option>}{snapshot.instances.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
      </select></label>}
    </div>
    {view === 'pack' && <section className="panel rocket-pack">
      <img src="/modbanner.png" alt="" />
      <div className="rocket-pack-content">
        <span className="tag">Recommended</span>
        <h2>Rocket Pack</h2>
        <p>{packVersion === recommendedVersion ? 'Fabric · 68 mods, resource packs, shaders, and Rocket Client · About 302 MiB on first download' : 'Fabric · Performance, visuals, and quality-of-life mods from Tuff, matched to your Minecraft version'}</p>
        <label className="field"><span>Rocket Pack Minecraft version</span><select value={packVersion} disabled={busy || installingPack} onChange={(event) => { setPackVersion(event.target.value); setPackInstalled(false); }}>
          <option value={recommendedVersion}>{recommendedVersion} · Full pack</option>
          {packVersions.map((version) => <option key={version.id} value={version.id}>{version.id} · Version-matched pack</option>)}
        </select></label>
        {packListError && !offline && <p>{packListError}</p>}
        <button type="button" className="button primary small-button" disabled={busy || offline || installingPack || packInstalled} onClick={() => void installPack()}>
          {installingPack ? <Spinner size={16} /> : <Download size={16} />}
          {installingPack ? 'Installing pack…' : packInstalled ? 'Instance created' : 'Create Rocket Pack instance'}
        </button>
        {offline && <p>Go online to download the pack.</p>}
        {packInstalled && <button className="button secondary small-button" onClick={onPlay}>Open Launch Pad</button>}
        {installingPack && packProgress && <div className="pack-progress">
          <p role="status">{packProgress[0]} of {packProgress[1]} pack steps complete</p>
          <progress className="game-progress" aria-label="Rocket Pack setup progress" value={packProgress[0]} max={packProgress[1]} />
        </div>}
      </div>
    </section>}
    {view === 'installed' && <section className="panel mods-panel">
      <div className="section-heading">
        <h2>Installed mods</h2>
        <div className="inline-actions">
          <button className="button secondary small-button" disabled={!id} onClick={() => void openFolder()}><FolderOpen size={16} /> Open mods folder</button>
          <button className="icon-button" title="Refresh mod list" aria-label="Refresh local mods" onClick={() => setRevision((value) => value + 1)} disabled={!id || loading || changingMod !== null}><RefreshCw size={16} /></button>
        </div>
      </div>
      {error && <p className="form-error" role="alert">{error}</p>}
      {running && <p className="panel-footnote">Close this instance before enabling or disabling mods.</p>}
      {loading ? <div className="loading-inline"><Spinner />Reading local files…</div> : id && mods.length ? <>
        <label className="search-field"><Search size={16} /><input aria-label="Filter installed mods" placeholder="Filter installed mods…" value={filter} onChange={(event) => setFilter(event.target.value)} /></label>
        <div className="mod-selection">
          <label><input type="checkbox" checked={visibleMods.length > 0 && visibleMods.every((mod) => selectedMods.has(mod.fileName))} disabled={visibleMods.length === 0 || changingMod !== null} onChange={(event) => {
            const checked = event.target.checked;
            setSelectedMods((previous) => {
              const next = new Set(previous);
              for (const mod of visibleMods) {
                if (checked) next.add(mod.fileName);
                else next.delete(mod.fileName);
              }
              return next;
            });
          }} /> Select visible mods</label>
          <span>{selectedMods.size} selected</span>
          <button className="button secondary small-button" disabled={busy || running || changingMod !== null || !mods.some((mod) => selectedMods.has(mod.fileName) && !mod.enabled)} onClick={() => void changeSelected(true)}>Enable selected</button>
          <button className="button secondary small-button" disabled={busy || running || changingMod !== null || !mods.some((mod) => selectedMods.has(mod.fileName) && mod.enabled)} onClick={() => void changeSelected(false)}>Disable selected</button>
        </div>
        {visibleMods.length ? <ul className="mod-list">{visibleMods.map((mod) => <li key={`${mod.fileName}:${mod.enabled}`}>
          <input type="checkbox" aria-label={`Select ${mod.fileName}`} checked={selectedMods.has(mod.fileName)} disabled={changingMod !== null} onChange={(event) => selectMod(mod.fileName, event.target.checked)} /><FileArchive size={19} /><span>{mod.fileName}</span><span className="tag">{mod.enabled ? 'Enabled' : 'Disabled'}</span>
          <button className="button secondary small-button" disabled={busy || running || changingMod !== null} aria-label={`${mod.enabled ? 'Disable' : 'Enable'} ${mod.fileName}`} onClick={() => void toggleMod(mod)}>
            {changingMod === mod.fileName && <Spinner size={14} />}{mod.enabled ? 'Disable' : 'Enable'}
          </button>
        </li>)}</ul> : <p className="panel-footnote">No mods match your search.</p>}
      </> : !error && <EmptyState icon={<FileArchive size={26} />} title={id ? 'No local mod files' : 'No instance to inspect'} description={id ? 'Install a mod or add a JAR to this instance’s mods folder.' : 'Create an instance first to inspect its local mods folder.'} />}
    </section>}
    {view === 'browse' && (offline ? <section className="panel mods-panel"><EmptyState icon={<Puzzle size={26} />} title="Modrinth is offline" description="Turn off Offline Mode in Settings to browse and install mods." /></section>
      : <ModrinthBrowser instance={instance?.loader !== 'vanilla' ? instance : undefined}
          gameVersions={[...new Set((catalog?.versions ?? []).filter((version) => version.type === 'release').map((version) => version.id).concat(snapshot.instances.map((item) => item.version)))]}
          onInstalled={() => setRevision((value) => value + 1)} onPackInstalled={() => onModpackInstalled?.()} />)}
  </>;
}
