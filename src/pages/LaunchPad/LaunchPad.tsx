import { ArrowRight, Box, Cpu, Download, Pencil, Plus, Puzzle, Rocket, Settings2, Wrench } from 'lucide-react';
import type { AppSnapshot, GameStatus, Instance, Page } from '../../types';
import { loaderName } from '../../services/loaders';
import { readinessLabel } from '../../services/readiness';
import { Landscape } from '../../components/launcher/Landscape';
import { InstanceCard } from '../../components/instances/InstanceCard';
import { formatRam } from '../../components/ui/Primitives';

export function LaunchPad({ snapshot, busy, status, onCreate, onEdit, onSelect, onNavigate, onInstall, onIgnite, onCheck }: {
  snapshot: AppSnapshot; busy: boolean;
  status?: GameStatus; onInstall: (instance: Instance) => void; onIgnite: (instance: Instance) => void; onCheck: (instance: Instance) => void;
  onCreate: () => void; onEdit: (instance: Instance) => void; onSelect: (instance: Instance) => void; onNavigate: (page: Page) => void;
}) {
  const selected = snapshot.instances.find((instance) => instance.id === snapshot.activeInstanceId);
  const readiness = selected?.readiness ?? 'checking';
  const recent = [...snapshot.instances].sort((a, b) => b.lastSelectedAt - a.lastSelectedAt).slice(0, 3);
  const active = ['launching', 'running', 'downloading', 'installing'].includes(readiness);

  const action = !selected ? { label: 'Create instance', icon: Plus, run: onCreate } :
    readiness === 'requires_download' ? { label: 'Install Minecraft', icon: Download, run: () => onInstall(selected) } :
    readiness === 'java_required' ? { label: 'Configure Java', icon: Settings2, run: () => onEdit(selected) } :
    readiness === 'authentication_required' ? { label: 'Set up a player profile', icon: Settings2, run: () => onNavigate('settings') } :
    readiness === 'ready' || readiness === 'missing_files' ? { label: 'Play Minecraft', icon: Rocket, run: () => onIgnite(selected) } :
    readiness === 'error' ? { label: 'Check again', icon: Settings2, run: () => onCheck(selected) } :
    { label: readinessLabel[readiness], icon: Rocket, run: undefined };
  const ActionIcon = action.icon;

  return <>
    <section className="play-panel" aria-label={selected ? 'Selected instance' : 'Getting started'}>
      <Landscape />
      <div className="play-content">
        <div className="play-context">
          {selected ? <label className="play-picker">
            <span>Selected instance</span>
            <select value={selected.id} disabled={busy} onChange={(event) => {
              const instance = snapshot.instances.find((item) => item.id === event.target.value);
              if (instance) onSelect(instance);
            }}>
              {snapshot.instances.map((instance) => <option key={instance.id} value={instance.id}>{instance.name}</option>)}
            </select>
          </label> : <span className="eyebrow">MINECRAFT JAVA EDITION</span>}
          {selected && <button className="icon-button play-edit" title="Edit instance" aria-label="Edit selected instance" disabled={busy} onClick={() => onEdit(selected)}><Pencil size={16} /></button>}
        </div>
        <h2>{selected?.name ?? 'Set up your first instance'}</h2>
        {selected ? <>
          <div className="play-meta">
            <span><Box size={15} />Minecraft {selected.version}</span>
            <span><Puzzle size={15} />{loaderName(selected.loader)}{selected.loaderVersion ? ` ${selected.loaderVersion}` : ''}</span>
            <span><Cpu size={15} />{formatRam(selected.ramMb)}</span>
          </div>
          <div className="play-status">
            <span className={`play-state ${readiness === 'ready' || readiness === 'running' ? 'is-ready' : ''}`}><span className="status-dot" />{readinessLabel[readiness]}</span>
            {status?.message && <p id="launch-status" role="status">{status.message}{status.exitCode !== null && ` Exit code: ${status.exitCode}`}</p>}
          </div>
        </> : <p className="play-empty">Choose a Minecraft version and mod loader, or start with Rocket Pack.</p>}
        <div className="play-actions">
          <button className="button primary" disabled={!action.run || busy} onClick={action.run} aria-describedby={status?.message ? 'launch-status' : undefined}><ActionIcon size={18} />{action.label}</button>
          {selected && readiness !== 'requires_download' && <button className="button secondary play-secondary" disabled={busy || active} onClick={() => onInstall(selected)}><Wrench size={16} />Repair files</button>}
          {!selected && <button className="button secondary play-secondary" onClick={() => onNavigate('mods')}><Puzzle size={16} />Browse Rocket Pack</button>}
        </div>
      </div>
    </section>

    {recent.length > 0 && <section className="recent-section">
      <div className="section-heading">
        <div><h2>Recent instances</h2><p>Switch versions or pick up where you left off.</p></div>
        <button className="button secondary small-button" onClick={() => onNavigate('instances')}>Manage instances <ArrowRight size={15} /></button>
      </div>
      <div className="instance-grid">{recent.map((instance) => <InstanceCard key={instance.id} instance={instance} compact active={instance.id === snapshot.activeInstanceId} busy={busy} onSelect={() => onSelect(instance)} onEdit={() => onEdit(instance)} />)}</div>
    </section>}
  </>;
}
