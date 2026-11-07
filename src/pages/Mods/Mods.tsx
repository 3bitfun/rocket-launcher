import { useEffect, useState } from 'react';
import { Download, FileArchive, Puzzle, RefreshCw } from 'lucide-react';
import type { AppSnapshot, InstalledMod, VersionCatalog } from '../../types';
import { native, errorMessage } from '../../services/native';
import { EmptyState, Spinner } from '../../components/ui/Primitives';
import { ModrinthBrowser } from './ModrinthBrowser';

const recommendedVersion = '1.21.11';

export function Mods({ snapshot, catalog, offline, busy, onPlay, onInstallPack }: { snapshot: AppSnapshot; catalog: VersionCatalog | null; offline: boolean; busy: boolean; onPlay: () => void; onInstallPack: (version: string, progress: (files: [number, number]) => void) => Promise<boolean> }) {
  const [view, setView] = useState<'browse' | 'installed' | 'pack'>(snapshot.instances.length ? 'browse' : 'pack');
  const [id, setId] = useState(snapshot.activeInstanceId ?? '');
  const [mods, setMods] = useState<InstalledMod[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [revision, setRevision] = useState(0);
  const [installingPack, setInstallingPack] = useState(false);
  const [packInstalled, setPackInstalled] = useState(false);
  const [packProgress, setPackProgress] = useState<[number, number] | null>(null);
  const [packVersion, setPackVersion] = useState(recommendedVersion);
  const [fabricVersions, setFabricVersions] = useState<string[]>([]);
  const [packListError, setPackListError] = useState('');
  const packVersions = (catalog?.versions ?? []).filter((version) => version.type === 'release' && version.id !== recommendedVersion && fabricVersions.includes(version.id));
  const instance = snapshot.instances.find((item) => item.id === id);
  useEffect(() => {
    if (offline || view !== 'pack') return;
    let active = true;
    setPackListError('');
    void native.fabricGameVersions()
      .then((versions) => { if (active) setFabricVersions(versions); })
      .catch((reason) => { if (active) setPackListError(errorMessage(reason)); });
    return () => { active = false; };
  }, [offline, view]);
  useEffect(() => { setId(snapshot.activeInstanceId ?? ''); }, [snapshot.activeInstanceId]);
  useEffect(() => {
    let cancelled = false;
    if (!id || view !== 'installed') return;
    setLoading(true); setError(null); setMods([]);
    native.mods(id).then((value) => { if (!cancelled) setMods(value); }).catch((reason) => { if (!cancelled) setError(errorMessage(reason)); }).finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [id, revision, view]);

  async function installPack() {
    setInstallingPack(true);
    setPackProgress(null);
    try { setPackInstalled(await onInstallPack(packVersion, setPackProgress)); }
    finally { setInstallingPack(false); }
  }

  return <>
    <div className="mod-workspace-bar">
      <div className="segmented" role="group" aria-label="Mods view">
        <button type="button" aria-pressed={view === 'browse'} className={view === 'browse' ? 'active' : ''} onClick={() => setView('browse')}>Browse mods</button>
        <button type="button" aria-pressed={view === 'installed'} className={view === 'installed' ? 'active' : ''} onClick={() => setView('installed')}>Installed mods</button>
        <button type="button" aria-pressed={view === 'pack'} className={view === 'pack' ? 'active' : ''} onClick={() => setView('pack')}>Rocket Pack</button>
      </div>
      {view !== 'pack' && <label className="instance-target"><span>Target instance</span><select value={id} onChange={(e) => setId(e.target.value)} disabled={!snapshot.instances.length || busy}>
        {!id && <option value="">Select an instance</option>}{snapshot.instances.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
      </select></label>}
    </div>
    {view === 'pack' && <section className="panel rocket-pack">
      <img src="/modbanner.png" alt="" />
      <div className="rocket-pack-content">
        <span className="tag">Recommended</span>
        <h2>Rocket Pack</h2>
        <p>{packVersion === recommendedVersion ? 'Fabric · 69 mods, resource packs, shaders, and Rocket Launcher Client · About 304 MiB' : 'Fabric · A starter pack assembled from compatible performance mods'}</p>
        <label className="field"><span>Rocket Pack Minecraft version</span><select value={packVersion} disabled={busy || installingPack} onChange={(event) => { setPackVersion(event.target.value); setPackInstalled(false); }}>
          <option value={recommendedVersion}>{recommendedVersion} · Full pack</option>
          {packVersions.map((version) => <option key={version.id} value={version.id}>{version.id} · Starter pack</option>)}
        </select></label>
        {packListError && !offline && <p>{packListError}</p>}
        <button type="button" className="button primary small-button" disabled={busy || offline || installingPack || packInstalled} onClick={() => void installPack()}>
          {installingPack ? <Spinner size={16} /> : <Download size={16} />}
          {installingPack ? 'Installing pack…' : packInstalled ? 'Instance created' : 'Create Rocket Pack instance'}
        </button>
        {offline && <p>Go online to download the pack.</p>}
        {packInstalled && <button className="button secondary small-button" onClick={onPlay}>Open Launch Pad</button>}
        {installingPack && packProgress && <div className="pack-progress">
          <p role="status">{packProgress[0]} of {packProgress[1]} files verified</p>
          <progress className="game-progress" aria-label="Rocket Pack files verified" value={packProgress[0]} max={packProgress[1]} />
        </div>}
      </div>
    </section>}
    {view === 'installed' && <section className="panel mods-panel"><div className="section-heading"><h2>Installed mod files</h2><button className="icon-button" title="Refresh mod list" aria-label="Refresh local mods" onClick={() => setRevision((value) => value + 1)} disabled={!id || loading}><RefreshCw size={16} /></button></div>
      {loading ? <div className="loading-inline"><Spinner />Reading local files…</div> : error ? <p className="form-error" role="alert">{error}</p> : mods.length ? <ul className="mod-list">{mods.map((mod) => <li key={mod.fileName}><FileArchive size={19} /><span>{mod.fileName}</span><small>{(mod.sizeBytes / 1024).toFixed(1)} KB</small><span className="tag">Not verified</span></li>)}</ul> : <EmptyState icon={<FileArchive size={26} />} title={id ? 'No local mod files' : 'No instance to inspect'} description={id ? 'No .jar files were found in this instance’s mods folder.' : 'Create an instance first to inspect its local mods folder.'} />}
      <p className="panel-footnote">Fabric and Forge instances load mods from this folder. This list only reports files on disk; compatibility and mod metadata have not been verified.</p>
    </section>}
    {view === 'browse' && (offline ? <section className="panel mods-panel"><EmptyState icon={<Puzzle size={26} />} title="Modrinth is offline" description="Turn off Offline Mode in Settings to browse and install mods." /></section>
      : instance && instance.loader !== 'vanilla'
      ? <ModrinthBrowser key={`${instance.id}:${instance.version}:${instance.loader}:${instance.loaderVersion}`} instance={instance} onInstalled={() => setRevision((value) => value + 1)} />
      : <section className="panel mods-panel"><EmptyState icon={<Puzzle size={26} />} title="Choose a modded instance" description="Select a Fabric or Forge instance, or create one with Rocket Pack."><button className="button secondary" onClick={() => setView('pack')}>Browse Rocket Pack</button></EmptyState></section>)}
  </>;
}
