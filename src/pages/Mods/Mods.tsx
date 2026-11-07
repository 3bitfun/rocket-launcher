import { useEffect, useState } from 'react';
import { FileArchive, Puzzle, RefreshCw } from 'lucide-react';
import type { AppSnapshot, InstalledMod } from '../../types';
import { native, errorMessage } from '../../services/native';
import { EmptyState, Spinner } from '../../components/ui/Primitives';
import { ModrinthBrowser } from './ModrinthBrowser';

export function Mods({ snapshot, offline }: { snapshot: AppSnapshot; offline: boolean }) {
  const [id, setId] = useState(snapshot.activeInstanceId ?? '');
  const [mods, setMods] = useState<InstalledMod[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [revision, setRevision] = useState(0);
  const instance = snapshot.instances.find((item) => item.id === id);
  useEffect(() => {
    let cancelled = false;
    if (!id) return;
    setLoading(true); setError(null); setMods([]);
    native.mods(id).then((value) => { if (!cancelled) setMods(value); }).catch((reason) => { if (!cancelled) setError(errorMessage(reason)); }).finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [id, revision]);
  return <>
    <div className="page-intro"><div><h2>Make your world a little more you.</h2><p>Find compatible mods and install them into the selected instance.</p></div></div>
    <section className="panel mods-panel"><div className="section-heading"><h2>Local mod files</h2><div className="inline-actions"><select aria-label="Instance to inspect mods" value={id} onChange={(e) => setId(e.target.value)} disabled={!snapshot.instances.length}>{!id && <option value="">No instance selected</option>}{snapshot.instances.map((instance) => <option key={instance.id} value={instance.id}>{instance.name}</option>)}</select><button className="icon-button" aria-label="Refresh local mods" onClick={() => setRevision((value) => value + 1)} disabled={!id || loading}><RefreshCw size={16} /></button></div></div>
      {loading ? <div className="loading-inline"><Spinner />Reading local files…</div> : error ? <p className="form-error" role="alert">{error}</p> : mods.length ? <ul className="mod-list">{mods.map((mod) => <li key={mod.fileName}><FileArchive size={19} /><span>{mod.fileName}</span><small>{(mod.sizeBytes / 1024).toFixed(1)} KB</small><span className="tag">Not verified</span></li>)}</ul> : <EmptyState icon={<FileArchive size={26} />} title={id ? 'No local mod files' : 'No instance to inspect'} description={id ? 'No .jar files were found in this instance’s mods folder.' : 'Create an instance first to inspect its local mods folder.'} />}
      <p className="panel-footnote">Only Fabric instances load mods from this folder. This list only reports files on disk; compatibility and mod metadata have not been verified.</p>
    </section>
    {offline ? <section className="panel mods-panel"><EmptyState icon={<Puzzle size={26} />} title="Modrinth is offline" description="Turn off Offline Mode in Settings to browse and install mods." /></section>
      : instance?.loader === 'fabric'
      ? <ModrinthBrowser key={`${instance.id}:${instance.version}:${instance.loaderVersion}`} instance={instance} onInstalled={() => setRevision((value) => value + 1)} />
      : <section className="panel mods-panel"><EmptyState icon={<Puzzle size={26} />} title="Browse Modrinth with Fabric" description="Create or select a Fabric instance to find mods for its Minecraft version." /></section>}
  </>;
}
