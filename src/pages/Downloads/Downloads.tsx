import { AlertCircle, CheckCircle2, CloudDownload, Database, Download, RefreshCw } from 'lucide-react';
import { EmptyState, Spinner, formatDate } from '../../components/ui/Primitives';
import type { VersionCatalog } from '../../types';

export function Downloads({ catalog, error, fetching, onRefresh }: { catalog: VersionCatalog | null; error: string | null; fetching: boolean; onRefresh: () => void }) {
  const warning = error || catalog?.warning;
  return <>
    <div className="page-intro"><div><h2>A little fuel for the journey.</h2><p>Real activity from your launcher, all in one place.</p></div><button className="button secondary" onClick={onRefresh} disabled={fetching}>{fetching ? <Spinner size={16} /> : <RefreshCw size={16} />}Refresh catalogue</button></div>
    <section className="panel catalogue-panel"><div className="section-heading"><h2>Version catalogue</h2><span className="tag">Mojang</span></div>
      <div className="catalogue-activity"><span className="activity-icon">{fetching ? <Spinner size={23} /> : warning ? <AlertCircle size={23} /> : catalog ? <CheckCircle2 size={23} /> : <CloudDownload size={23} />}</span><div><h3>{fetching ? 'Fueling up…' : warning ? 'Could not refresh the catalogue' : catalog ? 'Version catalogue available' : 'Waiting for the catalogue'}</h3><p>{fetching ? 'Fetching the official Minecraft version manifest.' : catalog ? `${catalog.versions.length} releases and snapshots · ${catalog.source === 'cache' ? 'Loaded from local cache' : 'Fetched from Mojang'}` : 'Refresh to retrieve official releases and snapshots.'}</p>{catalog && <small>Last fetched {formatDate(catalog.fetchedAt)}</small>}</div><span className={`tag ${warning ? 'warning-tag' : ''}`}>{fetching ? 'Fetching' : warning ? 'Retry available' : catalog ? 'Complete' : 'Pending'}</span></div>
      {fetching && <div className="indeterminate-track" role="status" aria-label="Fetching version catalogue"><span /></div>}
      {warning && <div className="notice warning" role="alert"><AlertCircle size={17} /><p>{warning}</p></div>}
      <div className="catalogue-source"><Database size={14} /><span>piston-meta.mojang.com</span><span>Cached on your computer</span></div>
    </section>
    <section className="panel downloads-empty"><EmptyState icon={<Download size={29} />} title="No game downloads yet" description="Minecraft installation, file verification, download progress, and retry controls are the next milestone."><span className="phase-badge">Available in Phase 2</span></EmptyState></section>
  </>;
}
