import { useEffect, useState } from 'react';
import { Check, Copy, Download, FolderOpen, Pencil, Puzzle, Rocket, Square, Terminal, Trash2, Wrench } from 'lucide-react';
import { errorMessage, native } from '../../services/native';
import { loaderName } from '../../services/loaders';
import { isGameActive, readinessLabel } from '../../services/readiness';
import type { GameStatus, Instance, RocketClientStatus, Toast } from '../../types';
import { Modal } from '../ui/Modal';
import { formatRam, Spinner } from '../ui/Primitives';
import { InstanceLog } from './InstanceLog';

interface Props {
  instance: Instance;
  status: GameStatus;
  active: boolean;
  offline: boolean;
  synced: boolean;
  busy: boolean;
  notice: Toast | null;
  stopping: boolean;
  onClose: () => void;
  onPlay: () => void;
  onStop: () => void;
  onInstall: () => void;
  onCheck: () => void;
  onProfile: () => void;
  onActivate: () => void;
  onEdit: () => void;
  onMods: () => void;
  onDuplicate: () => void;
  onRemove: () => void;
}

const clientAction = {
  available: 'Install Rocket Client',
  disabled: 'Enable Rocket Client',
  upgrade: 'Update Rocket Client',
} as const;

export function InstanceManager({ instance, status, active, offline, synced, busy, notice, stopping, onClose, onPlay, onStop, onInstall,
  onCheck, onProfile, onActivate, onEdit, onMods, onDuplicate, onRemove }: Props) {
  const [client, setClient] = useState<RocketClientStatus | null>(null);
  const [error, setError] = useState('');
  const [clientError, setClientError] = useState('');
  const [installing, setInstalling] = useState(false);
  const [logsOpen, setLogsOpen] = useState(false);
  const { id, loader, loaderVersion, version } = instance;
  const running = isGameActive(status.readiness);
  const locked = busy || installing;
  const needsInstall = status.readiness === 'requires_download';
  const canPlay = status.readiness === 'ready' || status.readiness === 'missing_files';
  const primary = canPlay ? { label: 'Play Minecraft', icon: Rocket, run: onPlay }
    : needsInstall ? instance.rocketPack
      ? { label: 'Install & play', icon: Rocket, run: onPlay }
      : { label: 'Install Minecraft', icon: Download, run: onInstall }
    : status.readiness === 'java_required' ? { label: 'Configure Java', icon: Pencil, run: onEdit }
    : status.readiness === 'authentication_required' ? { label: 'Set up player profile', icon: Pencil, run: onProfile }
    : status.readiness === 'error' ? { label: 'Check again', icon: Wrench, run: onCheck }
    : null;
  const PrimaryIcon = primary?.icon;

  useEffect(() => {
    let cancelled = false;
    setClient(null);
    setClientError('');
    void native.rocketClientStatus(id).then((result) => {
      if (!cancelled) setClient(result);
    }).catch((reason) => {
      if (!cancelled) setClientError(errorMessage(reason));
    });
    return () => { cancelled = true; };
  }, [id, loader, loaderVersion, version]);

  async function installClient() {
    setInstalling(true);
    setClientError('');
    try {
      await native.installRocketClient(id);
      setClient(await native.rocketClientStatus(id));
    } catch (reason) {
      setClientError(errorMessage(reason));
    } finally {
      setInstalling(false);
    }
  }

  async function openGameFolder() {
    setError('');
    try {
      await native.openGameFolder(id);
    } catch (reason) {
      setError(errorMessage(reason));
    }
  }

  return <Modal title={instance.name} description="Instance manager" onClose={onClose} busy={installing} wide>
    <div className="instance-manager">
      <div className="manager-status">
        <div>
          <div className="manager-tags">
            <span className="tag">Minecraft {version}</span>
            <span className="tag">{loaderName(loader)}{loaderVersion && ` ${loaderVersion}`}</span>
            <span className="tag">{formatRam(instance.ramMb)} RAM</span>
            {active && <span className="tag"><Check size={13} /> Active on Launch Pad</span>}
          </div>
          <h3>{readinessLabel[status.readiness]}</h3>
          <p role="status">{status.message}{status.exitCode !== null && ` Exit code ${status.exitCode}.`}</p>
        </div>
        {running ? <button className="button danger" onClick={onStop} disabled={stopping}>
          <Square size={16} />{stopping ? 'Stopping…' : status.readiness === 'running' ? 'Stop Minecraft' : 'Abort launch'}
        </button> : primary && PrimaryIcon && <button className="button primary" disabled={locked || (needsInstall && offline)} onClick={primary.run}>
          <PrimaryIcon size={17} />{primary.label}
        </button>}
      </div>
      <section className="manager-section" aria-label="Rocket Client">
        <span className="manager-icon"><Rocket size={19} /></span>
        <div className="manager-section-content">
          <h3>Rocket Client {client?.state === 'ready' && <span className="tag"><Check size={12} /> Installed</span>}</h3>
          <p>{client?.message ?? (clientError ? 'Could not read Rocket Client status.' : 'Checking compatibility…')}</p>
          {client?.state === 'ready' && <p>{synced ? 'HUD modules and layout sync across compatible instances.' : 'HUD modules and layout are saved for this instance.'}</p>}
          {client?.needsDownload && offline && <p>Go online to download Fabric API.</p>}
          {running && client && client.state in clientAction && <p>Close Minecraft to change the client mod.</p>}
          {clientError && <p className="form-error" role="alert">{clientError}</p>}
        </div>
        {client && client.state in clientAction && <button className="button secondary small-button" disabled={locked || running || (offline && client.needsDownload)} onClick={() => void installClient()}>
          {installing ? <Spinner size={15} /> : <Download size={15} />}
          {installing ? 'Installing…' : clientAction[client.state as keyof typeof clientAction]}
        </button>}
      </section>
      <section className="manager-section" aria-label="Game files">
        <span className="manager-icon"><FolderOpen size={19} /></span>
        <div className="manager-section-content">
          <h3>Game files</h3>
          <p className="manager-path">{instance.gameDirectory}</p>
          {error && <p className="form-error" role="alert">{error}</p>}
        </div>
        <button className="button secondary small-button" onClick={() => void openGameFolder()}>Open folder</button>
      </section>
      <section className="manager-section" aria-label="Minecraft session log">
        <span className="manager-icon"><Terminal size={19} /></span>
        <div className="manager-section-content">
          <h3>Minecraft log</h3>
          <p>View output from this instance’s latest Minecraft session.</p>
        </div>
        <button className="button secondary small-button" aria-expanded={logsOpen} aria-controls="instance-log-content" onClick={() => setLogsOpen(!logsOpen)}>{logsOpen ? 'Hide log' : 'View log'}</button>
        <div className="manager-log-content" id="instance-log-content" hidden={!logsOpen}>{logsOpen && <InstanceLog id={id} />}</div>
      </section>
      <div className="manager-actions">
        {!active && <button className="button secondary small-button" disabled={locked} onClick={onActivate}><Check size={15} /> Make active</button>}
        <button className="button secondary small-button" disabled={locked || running} onClick={onEdit}><Pencil size={15} /> Edit settings</button>
        {loader !== 'vanilla' && <button className="button secondary small-button" disabled={installing} onClick={onMods}><Puzzle size={15} /> Manage mods</button>}
        {!needsInstall && <button className="button secondary small-button" disabled={locked || running} onClick={onInstall}><Wrench size={15} /> Repair files</button>}
        <button className="button secondary small-button" disabled={locked} onClick={onDuplicate}><Copy size={15} /> Duplicate</button>
        <button className="button danger small-button" disabled={locked || running} onClick={onRemove}><Trash2 size={15} /> Remove</button>
      </div>
      {notice && <p className={notice.kind === 'error' ? 'form-error' : 'panel-footnote'} role={notice.kind === 'error' ? 'alert' : 'status'}>{notice.message}</p>}
    </div>
  </Modal>;
}
