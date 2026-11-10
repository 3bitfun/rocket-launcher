import { ArrowRight, Box, Cpu, Download, Plus, Puzzle, Rocket, Settings2, Square, Terminal, Users, Wrench } from 'lucide-react';
import type { AppSnapshot, GameStatus, Instance, Page, VersionCatalog } from '../../types';
import { loaderName, recommendedRocketVersion, rocketInstallLabel, rocketInstancesByVersion } from '../../services/loaders';
import { isGameActive, readinessLabel } from '../../services/readiness';
import { InstanceCard } from '../../components/instances/InstanceCard';
import { formatRam } from '../../components/ui/Primitives';
import { FlightScene } from '../../components/launcher/FlightScene';

const shortcuts = [
  { page: 'multiplayer', icon: Users, title: 'Multiplayer', detail: 'Join or share a world' },
  { page: 'mods', icon: Puzzle, title: 'Mods & packs', detail: 'Find your next setup' },
  { page: 'downloads', icon: Download, title: 'Downloads', detail: 'Check installation progress' },
] as const;

const defaultRocket = 'default-rocket';

export function LaunchPad({ snapshot, busy, catalog, rocketMode, rocketVersion, packProgress, fabricVersions, packCatalogueError, onRetryPackCatalogue, offline = false, stopping = false, status, onCreate, onEdit, onManage = onEdit, onSelect, onNavigate, onInstall, onIgnite, onAbort = () => {}, onChooseRocket, onSwitchVersion, onPlayNewRocket, onFavorite, onCheck, onDiagnose }: {
  snapshot: AppSnapshot; busy: boolean;
  catalog?: VersionCatalog | null;
  status?: GameStatus; onInstall: (instance: Instance) => void; onIgnite: (instance: Instance) => void; onCheck: (instance: Instance) => void;
  onAbort?: (instance: Instance) => void;
  rocketMode?: 'choose' | 'selected';
  rocketVersion?: string;
  packProgress?: [number, number] | null;
  fabricVersions?: string[] | null;
  packCatalogueError?: boolean;
  onRetryPackCatalogue?: () => void;
  offline?: boolean;
  stopping?: boolean;
  onChooseRocket?: () => void;
  onSwitchVersion?: (version: string) => void;
  onPlayNewRocket?: () => void;
  onFavorite?: (instance: Instance) => void;
  onDiagnose?: (instance: Instance) => void;
  onCreate: () => void; onEdit: (instance: Instance) => void; onManage?: (instance: Instance) => void; onSelect: (instance: Instance) => void; onNavigate: (page: Page) => void;
}) {
  const choosingVersion = rocketMode === 'choose';
  const selected = choosingVersion ? undefined : snapshot.instances.find((instance) => instance.id === snapshot.activeInstanceId);
  const localVersions = rocketInstancesByVersion(snapshot.instances);
  const compatible = new Set(fabricVersions);
  const versions = new Set(offline ? localVersions.keys() : [
    recommendedRocketVersion,
    ...(catalog?.versions ?? []).filter((entry) => entry.type === 'release' && compatible.has(entry.id)).map((entry) => entry.id),
    ...localVersions.keys(),
  ]);
  const readiness = selected?.readiness ?? 'checking';
  const favorites = snapshot.instances.filter((instance) => instance.favorite);
  const otherInstances = snapshot.instances.filter((instance) => !instance.favorite);
  const recent = [...snapshot.instances].sort((a, b) => Number(Boolean(b.favorite)) - Number(Boolean(a.favorite))
    || b.lastSelectedAt - a.lastSelectedAt).slice(0, 3);
  const active = isGameActive(readiness);

  const action = !selected ? { label: 'Create instance', icon: Plus, run: onCreate } :
    readiness === 'requires_download' ? selected.rocketPack
      ? { label: 'Install & play', icon: Rocket, run: offline ? undefined : () => onIgnite(selected) }
      : { label: 'Install Minecraft', icon: Download, run: () => onInstall(selected) } :
    readiness === 'java_required' ? { label: 'Configure Java', icon: Settings2, run: () => onEdit(selected) } :
    readiness === 'authentication_required' ? { label: 'Set up a player profile', icon: Settings2, run: () => onNavigate('settings') } :
    readiness === 'ready' || readiness === 'missing_files' ? { label: 'Play Minecraft', icon: Rocket, run: () => onIgnite(selected) } :
    readiness === 'error' ? { label: 'Check again', icon: Settings2, run: () => onCheck(selected) } :
    { label: readinessLabel[readiness], icon: Rocket, run: undefined };
  const ActionIcon = action.icon;

  return <>
    <section className="play-panel" aria-label={selected ? 'Selected instance' : 'Getting started'}>
      <FlightScene readiness={choosingVersion && packProgress ? 'downloading' : selected?.readiness} />
      <div className="play-content">
        <span className="play-kicker"><Rocket size={13} /> ROCKET LAUNCH PAD</span>
        <div className="play-context">
          {(selected || onChooseRocket) ? <label className="play-picker">
            <span>Selected instance</span>
            <select value={rocketMode ? defaultRocket : selected?.id ?? ''} disabled={busy} onChange={(event) => {
              if (event.target.value === defaultRocket) {
                onChooseRocket?.();
                return;
              }
              const instance = snapshot.instances.find((item) => item.id === event.target.value);
              if (instance) onSelect(instance);
            }}>
              {!selected && !rocketMode && <option value="" disabled>Choose an instance…</option>}
              {onChooseRocket && <option value={defaultRocket}>Default (Rocket)</option>}
              {favorites.length > 0 && <optgroup label="Favorites">{favorites.map((instance) => <option key={instance.id} value={instance.id}>{instance.name}</option>)}</optgroup>}
              {otherInstances.map((instance) => <option key={instance.id} value={instance.id}>{instance.name}</option>)}
            </select>
          </label> : <span className="play-edition">Minecraft Java Edition</span>}
          {selected && <button className="icon-button play-edit" title="Manage instance" aria-label="Manage selected instance" aria-haspopup="dialog" onClick={() => onManage(selected)}><Settings2 size={16} /></button>}
          {rocketMode && onSwitchVersion && <label className="play-picker">
            <span>Version</span>
            <select value={rocketVersion && versions.has(rocketVersion) ? rocketVersion : ''} disabled={busy || !versions.size} onChange={(event) => onSwitchVersion(event.target.value)}>
              <option value="" disabled>{versions.size ? 'Choose a Minecraft version…' : 'No local Rocket Packs'}</option>
              {[...versions].map((version) => <option key={version} value={version}>{version} · {rocketInstallLabel(localVersions.get(version))}{version === recommendedRocketVersion ? ' · Recommended' : ''}</option>)}
            </select>
          </label>}
          {rocketMode && rocketVersion && versions.has(rocketVersion) && <span className="tag" role="status">{rocketInstallLabel(selected ?? localVersions.get(rocketVersion))}</span>}
        </div>
        {rocketMode && !offline && packCatalogueError && <p className="play-empty" role="status">Could not load additional Fabric versions. <button className="button secondary small-button" type="button" onClick={onRetryPackCatalogue}>Retry</button></p>}
        <h2 className="play-title" key={selected?.id ?? rocketMode ?? 'first-instance'}>{selected?.name ?? (choosingVersion ? `Rocket Pack${rocketVersion ? ` ${rocketVersion}` : ''}` : 'Set up your first instance')}</h2>
        {selected ? <>
          <div className="play-meta">
            <span><Box size={15} />Minecraft {selected.version}</span>
            <span><Puzzle size={15} />{loaderName(selected.loader)}{selected.loaderVersion ? ` ${selected.loaderVersion}` : ''}</span>
            <span><Cpu size={15} />{formatRam(selected.ramMb)}</span>
          </div>
          <div className="play-status">
            <span className={`play-state ${readiness === 'ready' || readiness === 'running' ? 'is-ready' : ''}`}><span className="status-dot" />{readinessLabel[readiness]}</span>
            {status?.message && <p id="launch-status" role="status">{status.message}{status.exitCode !== null && ` Exit code: ${status.exitCode}`}</p>}
            {offline && selected.rocketPack && readiness === 'requires_download' && <p>Go online to finish installing this version.</p>}
          </div>
        </> : choosingVersion && packProgress ? <div className="play-status" role="status">
          <p>{packProgress[1] ? `Preparing Rocket Pack · ${packProgress[0]} of ${packProgress[1]} steps` : 'Preparing Rocket Pack…'}</p>
          <progress className="game-progress" aria-label="Rocket Pack setup" value={packProgress[1] ? packProgress[0] : undefined} max={packProgress[1] || 1} />
        </div> : <p className="play-empty">{choosingVersion
          ? offline ? 'Go online to set up a new Rocket Pack.' : 'Choose a version, then install and play. Rocket Pack, Minecraft, and Fabric are downloaded as needed.'
          : 'Choose a Minecraft version and mod loader, or start with Rocket Pack.'}</p>}
        <div className="play-actions">
          {active && selected ? <button className="button danger play-ignition" disabled={stopping} onClick={() => onAbort(selected)} aria-describedby={status?.message ? 'launch-status' : undefined}><Square size={16} />{stopping ? 'Stopping…' : readiness === 'running' ? 'Stop Minecraft' : 'Abort launch'}</button>
            : choosingVersion ? <button className="button primary play-ignition" disabled={busy || offline || !rocketVersion} onClick={onPlayNewRocket}><Rocket size={18} />Install &amp; play</button>
              : <button className={`button primary ${ActionIcon === Rocket ? 'play-ignition' : ''}`} disabled={!action.run || busy} onClick={action.run} aria-describedby={status?.message ? 'launch-status' : undefined}><ActionIcon size={18} />{action.label}</button>}
          {selected && readiness !== 'requires_download' && <button className="button secondary play-secondary" disabled={busy || active} onClick={() => onInstall(selected)}><Wrench size={16} />Repair files</button>}
          {selected && readiness === 'error' && onDiagnose && <button className="button secondary play-secondary" aria-haspopup="dialog" onClick={() => onDiagnose(selected)}><Terminal size={16} />Diagnose crash</button>}
          {!selected && <button className="button secondary play-secondary" onClick={() => onNavigate('mods')}><Puzzle size={16} />Browse Rocket Pack</button>}
        </div>
      </div>
    </section>

    <nav className="flight-shortcuts" aria-label="Launch Pad shortcuts">
      {shortcuts.map(({ page, icon: Icon, title, detail }) => <button key={page} type="button" className="flight-shortcut" onClick={() => onNavigate(page)}>
        <span className="flight-shortcut-icon"><Icon size={18} /></span>
        <span className="flight-shortcut-label"><strong>{title}</strong><small>{detail}</small></span>
        <ArrowRight size={14} className="flight-shortcut-arrow" />
      </button>)}
    </nav>

    {recent.length > 0 && <section className="recent-section">
      <div className="section-heading">
        <div><h2>{favorites.length ? 'Favorites & recent' : 'Recent instances'}</h2><p>Switch versions or pick up where you left off.</p></div>
        <button className="button secondary small-button" onClick={() => onNavigate('instances')}>Manage instances <ArrowRight size={15} /></button>
      </div>
      <div className="instance-grid">{recent.map((instance) => <InstanceCard key={instance.id} instance={instance} compact active={instance.id === snapshot.activeInstanceId} busy={busy} onManage={() => onManage(instance)} onEdit={() => onEdit(instance)} onFavorite={onFavorite && (() => onFavorite(instance))} />)}</div>
    </section>}
  </>;
}
