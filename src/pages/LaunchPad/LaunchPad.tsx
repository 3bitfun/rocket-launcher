import { ArrowRight, ArrowUpRight, Box, Check, Cpu, HardDrive, Plus, Rocket, ShieldCheck, UserRound } from 'lucide-react';
import type { AppSnapshot, GameStatus, Instance, Page, VersionCatalog } from '../../types';
import { loaderName } from '../../services/loaders';
import { readinessLabel } from '../../services/readiness';
import { Landscape } from '../../components/launcher/Landscape';
import { InstanceCard } from '../../components/instances/InstanceCard';
import { formatRam } from '../../components/ui/Primitives';

export function LaunchPad({ snapshot, catalog, busy, status, onCreate, onEdit, onSelect, onNavigate, onInstall, onIgnite, onCheck }: {
  snapshot: AppSnapshot; catalog: VersionCatalog | null; busy: boolean;
  status?: GameStatus; onInstall: (instance: Instance) => void; onIgnite: (instance: Instance) => void; onCheck: (instance: Instance) => void;
  onCreate: () => void; onEdit: (instance: Instance) => void; onSelect: (instance: Instance) => void; onNavigate: (page: Page) => void;
}) {
  const selected = snapshot.instances.find((instance) => instance.id === snapshot.activeInstanceId);
  const readiness = selected?.readiness;
  const action = !selected ? { label: 'Create instance', run: onCreate } :
    readiness === 'requires_download' || readiness === 'missing_files' ? { label: readiness === 'missing_files' ? 'Repair files' : 'Install Minecraft', run: () => onInstall(selected) } :
    readiness === 'java_required' ? { label: 'Configure Java', run: () => onEdit(selected) } :
    readiness === 'ready' ? { label: 'IGNITE', run: () => onIgnite(selected) } :
    readiness === 'error' ? { label: 'Check again', run: () => onCheck(selected) } :
    { label: readiness === 'authentication_required' ? 'IGNITE' : readinessLabel[readiness ?? 'checking'], run: undefined };
  const recent = [...snapshot.instances].sort((a, b) => b.lastSelectedAt - a.lastSelectedAt).slice(0, 3);
  return <>
    <section className="hero"><Landscape /><div className="hero-shade" /><div className="hero-content">
      <span className="hero-eyebrow"><span />A WORLD OF POSSIBILITIES</span><h2>Beyond the<br />Built Limit<span>.</span></h2><p>Your worlds, your way.<br />A fresh space for your next adventure.</p>
      <button className="button hero-button" onClick={onCreate}><Plus size={17} />Create instance<ArrowUpRight size={16} /></button>
    </div><div className="hero-caption"><Box size={13} /> MINECRAFT JAVA EDITION</div></section>

    <section className="launch-console panel" aria-label="Selected instance">
      <div className="launch-details"><span className="eyebrow">ON THE LAUNCH PAD</span><h2>{selected?.name ?? 'Your next adventure awaits'}</h2><div className="launch-meta">
        <span><Box size={14} />{selected ? `Minecraft ${selected.version}` : 'No instance selected'}</span>{selected && <><span className="separator">/</span><span>{loaderName(selected.loader)}{selected.loaderVersion ? ` ${selected.loaderVersion}` : ''}</span><span className="separator">/</span><span><Cpu size={14} />{formatRam(selected.ramMb)}</span></>}
      </div><span className="readiness"><span className="status-dot" />{readiness ? readinessLabel[readiness] : 'Create an instance to get started'}</span>
      {status?.message && <p className="launch-status" id="launch-status" role="status">{status.message}{status.exitCode !== null && <span> Exit code: {status.exitCode}</span>}</p>}</div>
      <div className="launch-action"><button className={action.label === 'IGNITE' ? 'ignite-button' : 'ignite-button task'} disabled={!action.run || busy} onClick={action.run} aria-describedby={status?.message ? 'launch-status' : undefined}><Rocket size={18} />{action.label}<ArrowUpRight size={15} /></button>
      {readiness === 'authentication_required' && selected && <button className="text-button" disabled={busy} onClick={() => onCheck(selected)}>Verify files again</button>}</div>
    </section>

    <section className="recent-section"><div className="section-heading"><div><h2>Recent instances <span className="count-badge">{snapshot.instances.length}</span></h2><p>Pick up where your imagination left off.</p></div><button className="text-button" onClick={() => onNavigate('instances')}>View all<ArrowRight size={15} /></button></div>
      {recent.length ? <div className="instance-grid">{recent.map((instance) => <InstanceCard key={instance.id} instance={instance} compact active={instance.id === snapshot.activeInstanceId} busy={busy} onSelect={() => onSelect(instance)} onEdit={() => onEdit(instance)} />)}</div>
        : <button className="create-tile" onClick={onCreate}><span className="create-tile-icon"><Plus size={23} /></span><span><strong>A place to call your world</strong><small>Create your first Minecraft instance</small></span><ArrowRight size={18} /></button>}
    </section>

    <div className="info-grid"><div className="info-card"><span className="info-icon"><ShieldCheck size={19} /></span><div><strong>Yours, locally.</strong><p>Instance settings stay on your computer.<br />Each world gets its own directory.</p></div></div>
      <button className="info-card interactive" onClick={() => onNavigate('downloads')}><span className="info-icon"><HardDrive size={19} /></span><div><strong>{catalog ? `Minecraft ${catalog.latest.release}` : 'Straight from the source'}</strong><p>{catalog ? 'Latest release in your version catalogue.' : 'Versions come directly from Mojang.'}<br />{catalog ? 'Manage your catalogue in Downloads.' : 'Check the catalogue in Downloads.'}</p></div><ArrowUpRight size={16} /></button></div>
    <div className="page-footnote"><span><Check size={13} /> Local configuration ready</span><span><UserRound size={13} /> Sign in to launch Minecraft Java Edition</span></div>
  </>;
}
