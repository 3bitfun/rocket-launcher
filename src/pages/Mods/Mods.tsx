import { useEffect, useState } from 'react';
import { FileArchive, Puzzle, RefreshCw } from 'lucide-react';
import type { AppSnapshot, InstalledMod } from '../../types';
import { native, errorMessage } from '../../services/native';
import { EmptyState, Spinner } from '../../components/ui/Primitives';

export function Mods({ snapshot }: { snapshot: AppSnapshot }) {
  const [id, setId] = useState(snapshot.activeInstanceId ?? '');
  const [mods, setMods] = useState<InstalledMod[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    let cancelled = false;
    if (!id) return;
    setLoading(true); setError(null); setMods([]);
    native.mods(id).then((value) => { if (!cancelled) setMods(value); }).catch((reason) => { if (!cancelled) setError(errorMessage(reason)); }).finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [id, revision]);
  return <>
    <div className="page-intro"><div><h2>Make your world a little more you.</h2><p>A home for your mods. More possibilities on the horizon.</p></div><span className="phase-badge">Modding · Phase 4</span></div>
    <section className="panel integration-notice"><span className="empty-icon"><Puzzle size={26} /></span><div><h3>Modrinth integration is on the horizon.</h3><p>Browsing, installing, and resolving mod dependencies are unavailable in this build. Fabric, Forge, NeoForge, and modpacks are planned for Phase 4.</p></div><span className="tag">Unavailable</span></section>
    <section className="panel mods-panel"><div className="section-heading"><h2>Local mod files</h2><div className="inline-actions"><select aria-label="Instance to inspect mods" value={id} onChange={(e) => setId(e.target.value)} disabled={!snapshot.instances.length}>{!id && <option value="">No instance selected</option>}{snapshot.instances.map((instance) => <option key={instance.id} value={instance.id}>{instance.name}</option>)}</select><button className="icon-button" aria-label="Refresh local mods" onClick={() => setRevision((value) => value + 1)} disabled={!id || loading}><RefreshCw size={16} /></button></div></div>
      {loading ? <div className="loading-inline"><Spinner />Reading local files…</div> : error ? <p className="form-error" role="alert">{error}</p> : mods.length ? <ul className="mod-list">{mods.map((mod) => <li key={mod.fileName}><FileArchive size={19} /><span>{mod.fileName}</span><small>{(mod.sizeBytes / 1024).toFixed(1)} KB</small><span className="tag">Not verified</span></li>)}</ul> : <EmptyState icon={<FileArchive size={26} />} title={id ? 'No local mod files' : 'No instance to inspect'} description={id ? 'No .jar files were found in this instance’s mods folder.' : 'Create an instance first to inspect its local mods folder.'} />}
      <p className="panel-footnote">Vanilla Minecraft does not load mods. This list only reports files on disk; compatibility and mod metadata have not been verified.</p>
    </section>
  </>;
}
