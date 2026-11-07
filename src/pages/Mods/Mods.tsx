import { useEffect, useState } from 'react';
import { Download, FileArchive, Puzzle, RefreshCw } from 'lucide-react';
import type { AppSnapshot, InstalledMod, VersionCatalog } from '../../types';
import { native, errorMessage } from '../../services/native';
import { EmptyState, Spinner } from '../../components/ui/Primitives';
import { ModrinthBrowser } from './ModrinthBrowser';

const recommendedVersion = '1.21.11';

export function Mods({ snapshot, catalog, offline, busy, onInstallPack }: { snapshot: AppSnapshot; catalog: VersionCatalog | null; offline: boolean; busy: boolean; onInstallPack: (version: string) => Promise<boolean> }) {
  const [id, setId] = useState(snapshot.activeInstanceId ?? '');
  const [mods, setMods] = useState<InstalledMod[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [revision, setRevision] = useState(0);
  const [installingPack, setInstallingPack] = useState(false);
  const [packInstalled, setPackInstalled] = useState(false);
  const [packVersion, setPackVersion] = useState(recommendedVersion);
  const [fabricVersions, setFabricVersions] = useState<string[]>([]);
  const [packListError, setPackListError] = useState('');
  const packVersions = (catalog?.versions ?? []).filter((version) => version.type === 'release' && version.id !== recommendedVersion && fabricVersions.includes(version.id));
  const instance = snapshot.instances.find((item) => item.id === id);
  useEffect(() => {
    if (offline) return;
    let active = true;
    setPackListError('');
    void native.fabricGameVersions()
      .then((versions) => { if (active) setFabricVersions(versions); })
      .catch((reason) => { if (active) setPackListError(errorMessage(reason)); });
    return () => { active = false; };
  }, [offline]);
  useEffect(() => { setId(snapshot.activeInstanceId ?? ''); }, [snapshot.activeInstanceId]);
  useEffect(() => {
    let cancelled = false;
    if (!id) return;
    setLoading(true); setError(null); setMods([]);
    native.mods(id).then((value) => { if (!cancelled) setMods(value); }).catch((reason) => { if (!cancelled) setError(errorMessage(reason)); }).finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [id, revision]);

  async function installPack() {
    setInstallingPack(true);
    try { setPackInstalled(await onInstallPack(packVersion)); }
    finally { setInstallingPack(false); }
  }

  return <>
    <div className="page-intro"><div><h2>Make your world a little more you.</h2><p>Find compatible mods and install them into the selected instance.</p></div></div>
    <section className="panel rocket-pack">
      <img src="/modbanner.png" alt="" />
      <div className="rocket-pack-content">
        <span className="tag">Recommended</span>
        <h2>Rocket Pack</h2>
        <p>{packVersion === recommendedVersion ? 'Fabric · 18 pinned performance and interface mods' : 'Fabric · A starter pack assembled from compatible performance mods'}</p>
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
        {packInstalled && <p>Open Launch Pad to install the Minecraft files and play.</p>}
      </div>
    </section>
    <section className="panel mods-panel"><div className="section-heading"><h2>Local mod files</h2><div className="inline-actions"><select aria-label="Instance to inspect mods" value={id} onChange={(e) => setId(e.target.value)} disabled={!snapshot.instances.length}>{!id && <option value="">No instance selected</option>}{snapshot.instances.map((instance) => <option key={instance.id} value={instance.id}>{instance.name}</option>)}</select><button className="icon-button" aria-label="Refresh local mods" onClick={() => setRevision((value) => value + 1)} disabled={!id || loading}><RefreshCw size={16} /></button></div></div>
      {loading ? <div className="loading-inline"><Spinner />Reading local files…</div> : error ? <p className="form-error" role="alert">{error}</p> : mods.length ? <ul className="mod-list">{mods.map((mod) => <li key={mod.fileName}><FileArchive size={19} /><span>{mod.fileName}</span><small>{(mod.sizeBytes / 1024).toFixed(1)} KB</small><span className="tag">Not verified</span></li>)}</ul> : <EmptyState icon={<FileArchive size={26} />} title={id ? 'No local mod files' : 'No instance to inspect'} description={id ? 'No .jar files were found in this instance’s mods folder.' : 'Create an instance first to inspect its local mods folder.'} />}
      <p className="panel-footnote">Fabric and Forge instances load mods from this folder. This list only reports files on disk; compatibility and mod metadata have not been verified.</p>
    </section>
    {offline ? <section className="panel mods-panel"><EmptyState icon={<Puzzle size={26} />} title="Modrinth is offline" description="Turn off Offline Mode in Settings to browse and install mods." /></section>
      : instance && instance.loader !== 'vanilla'
      ? <ModrinthBrowser key={`${instance.id}:${instance.version}:${instance.loader}:${instance.loaderVersion}`} instance={instance} onInstalled={() => setRevision((value) => value + 1)} />
      : <section className="panel mods-panel"><EmptyState icon={<Puzzle size={26} />} title="Browse Modrinth with a modded instance" description="Create or select a Fabric or Forge instance to find mods for its Minecraft version." /></section>}
  </>;
}
