import { AlertCircle, CheckCircle2, CloudDownload, Database, Download, RefreshCw } from 'lucide-react';
import { EmptyState, Spinner, formatDate } from '../../components/ui/Primitives';
import type { DownloadProgress, VersionCatalog } from '../../types';

const mib = (bytes: number) => `${(bytes / 1048576).toFixed(1)} MiB`;

export function Downloads({ catalog, error, fetching, onRefresh, progress, onRetry }: { catalog: VersionCatalog | null; error: string | null; fetching: boolean; onRefresh: () => void; progress?: DownloadProgress | null; onRetry?: () => void }) {
  const warning = error || catalog?.warning;
  return <>
    <div className="page-intro"><div><h2>Installation activity</h2><p>View download progress or retry a failed installation.</p></div></div>
    {progress ? <section className="panel catalogue-panel download-activity" aria-label="Minecraft installation">
      <div className="section-heading"><h2>Minecraft {progress.version}</h2><span className="tag">{progress.stage === 'complete' ? 'Verified' : progress.stage === 'error' ? 'Failed' : progress.stage === 'installing' ? 'Installing' : 'Downloading'}</span></div>
      <p className="launch-status">{progress.current}</p>
      {progress.totalBytes && progress.stage !== 'error' ? <progress className="game-progress activity-progress" data-stage={progress.stage} aria-label="Verified Minecraft files" value={progress.verifiedBytes} max={progress.totalBytes} /> :
        progress.stage === 'downloading' ? <div className="indeterminate-track" role="status" aria-label="Resolving Minecraft download size"><span /></div> : null}
      <div className="catalogue-source"><span>{mib(progress.transferredBytes)} transferred{progress.stage === 'downloading' ? ` · ${mib(progress.bytesPerSecond)}/s` : ''}</span>
        {progress.totalBytes !== null && <span>{mib(progress.verifiedBytes)} / {mib(progress.totalBytes)} verified</span>}</div>
      {progress.error && <div className="notice warning" role="alert"><AlertCircle size={17} /><p>{progress.error}</p></div>}
      {progress.stage === 'error' && onRetry && <button className="button secondary" onClick={onRetry}><RefreshCw size={16} />Retry installation</button>}
    </section> : <section className="panel downloads-empty"><EmptyState icon={<Download size={29} />} title="No game downloads yet" description="Choose an instance on the Launch Pad to install Minecraft." /></section>}
    <section className="panel catalogue-panel downloads-empty"><div className="section-heading"><h2>Version catalogue</h2><button className="button secondary small-button" onClick={onRefresh} disabled={fetching}>{fetching ? <Spinner size={16} /> : <RefreshCw size={16} />}Refresh catalogue</button></div>
      <div className="catalogue-activity"><span className="activity-icon">{fetching ? <Spinner size={23} /> : warning ? <AlertCircle size={23} /> : catalog ? <CheckCircle2 size={23} /> : <CloudDownload size={23} />}</span><div><h3>{fetching ? 'Refreshing catalogue…' : warning ? 'Could not refresh the catalogue' : catalog ? 'Version catalogue available' : 'Waiting for the catalogue'}</h3><p>{fetching ? 'Fetching the official Minecraft version manifest.' : catalog ? `${catalog.versions.length} releases and snapshots · ${catalog.source === 'cache' ? 'Loaded from local cache' : 'Fetched from Mojang'}` : 'Refresh to retrieve official releases and snapshots.'}</p>{catalog && <small>Last fetched {formatDate(catalog.fetchedAt)}</small>}</div><span className={`tag ${warning ? 'warning-tag' : ''}`}>{fetching ? 'Fetching' : warning ? 'Retry available' : catalog ? 'Complete' : 'Pending'}</span></div>
      {fetching && <div className="indeterminate-track" role="status" aria-label="Fetching version catalogue"><span /></div>}
      {warning && <div className="notice warning" role="alert"><AlertCircle size={17} /><p>{warning}</p></div>}
      <div className="catalogue-source"><Database size={14} /><span>piston-meta.mojang.com</span><span>Cached on your computer</span></div>
    </section>
  </>;
}
