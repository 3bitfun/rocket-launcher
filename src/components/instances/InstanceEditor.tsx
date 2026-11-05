import { useMemo, useState, type FormEvent } from 'react';
import { AlertCircle, Box, RefreshCw } from 'lucide-react';
import { Modal } from '../ui/Modal';
import { PathField, Spinner, formatRam } from '../ui/Primitives';
import type { Instance, InstanceInput, Settings, VersionCatalog } from '../../types';

export function InstanceEditor({ instance, settings, catalog, fetching, catalogError, saveError, busy, onRefresh, onSave, onClose }: {
  instance?: Instance; settings: Settings; catalog: VersionCatalog | null; fetching: boolean; catalogError: string | null; saveError?: string | null; busy: boolean;
  onRefresh: () => void; onSave: (input: InstanceInput) => Promise<boolean>; onClose: () => void;
}) {
  const [name, setName] = useState(instance?.name ?? '');
  const [version, setVersion] = useState(instance?.version ?? '');
  const [ram, setRam] = useState(instance?.ramMb ?? settings.defaultRamMb);
  const [java, setJava] = useState(instance?.javaPath ?? '');
  const [parent, setParent] = useState(settings.minecraftDirectory);
  const [query, setQuery] = useState('');
  const [showSnapshots, setShowSnapshots] = useState(settings.showSnapshots);
  const [error, setError] = useState<string | null>(null);
  const selectedVersion = version || catalog?.latest.release || '';
  const versions = useMemo(() => catalog?.versions.filter((entry) => (showSnapshots || entry.type === 'release' || entry.id === selectedVersion) && entry.id.toLowerCase().includes(query.toLowerCase())) ?? [], [catalog, query, selectedVersion, showSnapshots]);
  const selectedMissing = selectedVersion && !versions.some((entry) => entry.id === selectedVersion);

  async function submit(event: FormEvent) {
    event.preventDefault(); setError(null);
    if (!name.trim()) { setError('Give your instance a name.'); return; }
    if (!selectedVersion) { setError('Fetch the version catalogue and choose a Minecraft version.'); return; }
    if (await onSave({ name: name.trim(), version: selectedVersion, ramMb: ram, javaPath: java.trim() || null, parentDirectory: instance ? null : parent.trim() })) onClose();
  }

  return <Modal title={instance ? 'Edit instance' : 'Create an instance'} description={instance ? 'Make this little world your own.' : 'Every adventure starts with a space of its own.'} onClose={onClose} busy={busy}>
    <form onSubmit={(event) => void submit(event)}>
      <fieldset disabled={busy} className="editor-fields">
        <label className="field"><span>Instance name</span><input autoFocus maxLength={64} value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Sunday survival" required /></label>
        <div className="field"><div className="field-title"><label htmlFor="mc-version">Minecraft version</label><button type="button" className="text-button" onClick={onRefresh} disabled={fetching}>{fetching ? <Spinner size={13} /> : <RefreshCw size={13} />} Refresh</button></div>
          <input aria-label="Filter Minecraft versions" placeholder="Find a version…" value={query} onChange={(e) => setQuery(e.target.value)} />
          <select id="mc-version" value={selectedVersion} onChange={(e) => setVersion(e.target.value)} required disabled={!catalog && !instance}>
            {!selectedVersion && <option value="">{fetching ? 'Loading official versions…' : 'Version catalogue unavailable'}</option>}
            {selectedMissing && <option value={selectedVersion}>{selectedVersion} (selected)</option>}
            {versions.map((entry) => <option key={entry.id} value={entry.id}>{entry.id}{entry.type === 'snapshot' ? ' · Snapshot' : ''}{entry.id === catalog?.latest.release ? ' · Latest release' : ''}</option>)}
          </select>
          <label className="checkbox-label"><input type="checkbox" checked={showSnapshots} onChange={(e) => setShowSnapshots(e.target.checked)} />Include snapshots</label>
          {(catalogError || catalog?.warning) && <small className="warning-text">{catalogError || catalog?.warning}</small>}
          <small>Official Mojang catalogue. Historical alpha and beta versions are not included.</small>
        </div>
        <div className="loader-option"><Box size={19} /><div><strong>Vanilla</strong><small>The original Minecraft experience</small></div><span className="tag">Selected</span></div>
        <label className="field"><span className="field-title">Memory allocation <strong>{formatRam(ram)}</strong></span><input type="range" min="1024" max="16384" step="256" value={Math.min(ram, 16384)} onChange={(e) => setRam(Number(e.target.value))} /><div className="ram-input"><input aria-label="Memory in MB" type="number" min="1024" max="65536" step="256" value={ram} onChange={(e) => setRam(Number(e.target.value))} required /><small>MB · leave memory for Windows and other apps</small></div></label>
        {instance ? <label className="field"><span>Game directory</span><input value={instance.gameDirectory} readOnly /><small>Existing directories stay in place. Create a new instance to choose another location.</small></label> : <PathField label="Instance parent directory" value={parent} onChange={setParent} onError={setError} hint="A unique game folder is created here. Your existing files stay untouched." />}
        <PathField label="Java executable override" kind="java" value={java} onChange={setJava} onError={setError} hint="Optional. Leave blank to use the launcher default or automatic detection." />
      </fieldset>
      <div className="notice"><AlertCircle size={16} /><span>Install or verify Minecraft files from the Launch Pad after saving.</span></div>
      {(error || saveError) && <p role="alert" className="form-error">{error || saveError}</p>}
      <div className="modal-actions"><button type="button" className="button secondary" onClick={onClose} disabled={busy}>Cancel</button><button className="button primary" type="submit" disabled={busy || !selectedVersion}>{busy && <Spinner />}{instance ? 'Save changes' : 'Create instance'}</button></div>
    </form>
  </Modal>;
}
