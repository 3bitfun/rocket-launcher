import { useEffect, useMemo, useState, type FormEvent } from 'react';
import { AlertCircle, Box, Puzzle, RefreshCw, Rocket } from 'lucide-react';
import { Modal } from '../ui/Modal';
import { PathField, Spinner, formatRam } from '../ui/Primitives';
import { loaderName } from '../../services/loaders';
import { errorMessage, native } from '../../services/native';
import type { FabricLoaderVersion, Instance, InstanceInput, Loader, Settings, VersionCatalog } from '../../types';

export function InstanceEditor({ instance, settings, catalog, fetching, catalogError, saveError, busy, offline = false, onRefresh, onSave, onClose }: {
  instance?: Instance; settings: Settings; catalog: VersionCatalog | null; fetching: boolean; catalogError: string | null; saveError?: string | null; busy: boolean;
  offline?: boolean;
  onRefresh: () => void; onSave: (input: InstanceInput, onPackProgress?: (progress: [number, number]) => void) => Promise<boolean>; onClose: () => void;
}) {
  const [name, setName] = useState(instance?.name ?? '');
  const [version, setVersion] = useState(instance?.version ?? '');
  const [loader, setLoader] = useState<Loader>(instance?.loader ?? 'fabric');
  const [rocketPack, setRocketPack] = useState(!instance);
  const [packProgress, setPackProgress] = useState<[number, number] | null>(null);
  const [loaderVersion, setLoaderVersion] = useState<string | null>(instance?.loaderVersion ?? null);
  const [fabricLoaders, setFabricLoaders] = useState<FabricLoaderVersion[] | null>(null);
  const [fabricError, setFabricError] = useState<string | null>(null);
  const [fabricLoading, setFabricLoading] = useState(false);
  const [fabricRevision, setFabricRevision] = useState(0);
  const [ram, setRam] = useState(instance?.ramMb ?? settings.defaultRamMb);
  const [java, setJava] = useState(instance?.javaPath ?? '');
  const [parent, setParent] = useState(`${settings.minecraftDirectory}\\Rocket`);
  const [query, setQuery] = useState('');
  const [showSnapshots, setShowSnapshots] = useState(settings.showSnapshots);
  const [error, setError] = useState<string | null>(null);
  const lockedPack = instance?.rocketPack === true;
  const selectedVersion = version || catalog?.latest.release || '';
  const defaultName = rocketPack && selectedVersion ? `Rocket Pack ${selectedVersion}` : '';
  const packUnavailable = rocketPack && (offline || catalog?.versions.find((entry) => entry.id === selectedVersion)?.type === 'snapshot');
  const needsLoader = !rocketPack && !lockedPack && loader !== 'vanilla';
  const versions = useMemo(() => catalog?.versions.filter((entry) => (showSnapshots || entry.type === 'release' || entry.id === selectedVersion) && entry.id.toLowerCase().includes(query.toLowerCase())) ?? [], [catalog, query, selectedVersion, showSnapshots]);
  const selectedMissing = selectedVersion && !versions.some((entry) => entry.id === selectedVersion);
  const loaderOptions = fabricLoaders ?? [];
  const loaderListed = !loaderVersion || loaderOptions.some((entry) => entry.version === loaderVersion);
  const loaderPlaceholder = fabricError
    ? 'Loader list unavailable'
    : fabricLoading || !fabricLoaders
      ? 'Loading compatible loaders…'
      : !fabricLoaders.length ? `No compatible loaders for ${selectedVersion || 'this version'}` : 'Choose a loader…';

  useEffect(() => {
    if (!needsLoader || !selectedVersion) return;
    let cancelled = false;
    setFabricLoading(true); setFabricError(null);
    (loader === 'forge' ? native.forgeLoaders(selectedVersion) : native.fabricLoaders(selectedVersion))
      .then((values) => {
        if (cancelled) return;
        setFabricLoaders(values);
        setLoaderVersion((current) => {
          if (current && (values.some((value) => value.version === current) ||
            (instance?.loader === loader && instance.version === selectedVersion && instance.loaderVersion === current))) return current;
          return (values.find((value) => value.stable) ?? values[0])?.version ?? null;
        });
      })
      .catch((reason) => {
        if (cancelled) return;
        setFabricLoaders([]);
        setLoaderVersion(instance?.loader === loader && instance.version === selectedVersion ? instance.loaderVersion ?? null : null);
        setFabricError(errorMessage(reason));
      })
      .finally(() => { if (!cancelled) setFabricLoading(false); });
    return () => { cancelled = true; };
  }, [needsLoader, loader, selectedVersion, fabricRevision, instance?.version, instance?.loaderVersion, instance?.loader]);

  function selectLoader(next: Loader) {
    setRocketPack(false);
    if (!instance) setParent(settings.minecraftDirectory);
    if (next === loader) return;
    setLoader(next);
    setLoaderVersion(instance?.loader === next && instance.version === selectedVersion ? instance.loaderVersion : null);
    setFabricLoaders(null);
    setFabricError(null);
  }

  async function submit(event: FormEvent) {
    event.preventDefault(); setError(null);
    const instanceName = name.trim() || defaultName;
    if (!instanceName) { setError('Give your instance a name.'); return; }
    if (!selectedVersion) { setError('Fetch the version catalogue and choose a Minecraft version.'); return; }
    if (packUnavailable) return;
    if (needsLoader && !loaderVersion) { setError('Choose a compatible mod loader version.'); return; }
    setPackProgress(null);
    const input: InstanceInput = {
      name: instanceName, version: selectedVersion, loader, loaderVersion: needsLoader || lockedPack ? loaderVersion : null,
      ramMb: ram, javaPath: java.trim() || null, parentDirectory: instance ? null : parent.trim(),
    };
    const saved = await (rocketPack ? onSave(input, setPackProgress) : onSave(input));
    if (saved) onClose();
  }

  return <Modal title={instance ? 'Edit instance' : 'Create an instance'} description={instance ? 'Make this little world your own.' : 'Every adventure starts with a space of its own.'} onClose={onClose} busy={busy}>
    <form onSubmit={(event) => void submit(event)}>
      <fieldset disabled={busy} className="editor-fields">
        <label className="field"><span>Instance name</span><input autoFocus maxLength={64} value={name} onChange={(e) => setName(e.target.value)} placeholder={defaultName || 'e.g. Sunday survival'} required={!rocketPack} /></label>
        {lockedPack ? <div className="notice"><Rocket size={18} /><span>Rocket Pack · Minecraft {selectedVersion} · Fabric {loaderVersion}. Switch versions from Default (Rocket) on the Launch Pad.</span></div> : <>
        <div className="field"><div className="field-title"><label htmlFor="mc-version">Minecraft version</label><button type="button" className="text-button" onClick={onRefresh} disabled={fetching}>{fetching ? <Spinner size={13} /> : <RefreshCw size={13} />} Refresh</button></div>
          <input aria-label="Filter Minecraft versions" placeholder="Find a version…" value={query} onChange={(e) => setQuery(e.target.value)} />
          <select id="mc-version" value={selectedVersion} onChange={(e) => {
            setVersion(e.target.value);
            setLoaderVersion(e.target.value === instance?.version && instance.loader === loader ? instance.loaderVersion ?? null : null);
            setFabricLoaders(null);
          }} required disabled={!catalog && !instance}>
            {!selectedVersion && <option value="">{fetching ? 'Loading official versions…' : 'Version catalogue unavailable'}</option>}
            {selectedMissing && <option value={selectedVersion}>{selectedVersion} (selected)</option>}
            {versions.map((entry) => <option key={entry.id} value={entry.id}>{entry.id}{entry.type === 'snapshot' ? ' · Snapshot' : ''}{entry.id === catalog?.latest.release ? ' · Latest release' : ''}</option>)}
          </select>
          <label className="checkbox-label"><input type="checkbox" checked={showSnapshots} onChange={(e) => setShowSnapshots(e.target.checked)} />Include snapshots</label>
          {(catalogError || catalog?.warning) && <small className="warning-text">{catalogError || catalog?.warning}</small>}
          <small>Official Mojang catalogue. Historical alpha and beta versions are not included.</small>
        </div>
        <div className="loader-grid" role="group" aria-label="Instance setup">
          {!instance && <button type="button" className={`loader-option ${rocketPack ? 'selected' : ''}`} onClick={() => {
            selectLoader('fabric');
            setRocketPack(true);
            setParent(`${settings.minecraftDirectory}\\Rocket`);
          }} aria-pressed={rocketPack}>
            <Rocket size={19} /><div><strong>Fabric Rocket Pack</strong><small>Recommended mods matched to your version</small></div>{rocketPack && <span className="tag">Selected</span>}
          </button>}
          <button type="button" className={`loader-option ${loader === 'vanilla' ? 'selected' : ''}`} onClick={() => selectLoader('vanilla')} aria-pressed={loader === 'vanilla'}>
            <Box size={19} /><div><strong>Vanilla</strong><small>The original Minecraft experience</small></div>{loader === 'vanilla' && <span className="tag">Selected</span>}
          </button>
          <button type="button" className={`loader-option ${loader === 'fabric' && !rocketPack ? 'selected' : ''}`} onClick={() => selectLoader('fabric')} aria-pressed={loader === 'fabric' && !rocketPack}>
            <Puzzle size={19} /><div><strong>Fabric</strong><small>Choose your own mods</small></div>{loader === 'fabric' && !rocketPack && <span className="tag">Selected</span>}
          </button>
          <button type="button" className={loader === 'forge' ? 'loader-option selected' : 'loader-option'} onClick={() => selectLoader('forge')} aria-pressed={loader === 'forge'}>
            <Puzzle size={19} /><div><strong>Forge</strong><small>Minecraft 1.20.1 and newer</small></div>{loader === 'forge' && <span className="tag">Selected</span>}
          </button>
        </div>
        {needsLoader && <div className="field">
          <div className="field-title"><label htmlFor="fabric-loader">{loaderName(loader)} loader</label><button type="button" className="text-button" onClick={() => setFabricRevision((value) => value + 1)} disabled={fabricLoading || !selectedVersion}>{fabricLoading ? <Spinner size={13} /> : <RefreshCw size={13} />} Refresh</button></div>
          <select id="fabric-loader" value={loaderVersion ?? ''} onChange={(e) => setLoaderVersion(e.target.value || null)} required disabled={fabricLoading}>
            {(!loaderVersion || loaderListed) && <option value="">{loaderPlaceholder}</option>}
            {!loaderListed && <option value={loaderVersion}>{loaderVersion} (selected)</option>}
            {loaderOptions.map((entry) => <option key={entry.version} value={entry.version}>{entry.version}{entry.stable ? ' · Recommended' : ' · Latest available'}</option>)}
          </select>
          {fabricError && <small className="warning-text">{fabricError}</small>}
          {!fabricError && <small>The recommended loader is preselected when available.</small>}
        </div>}
        </>}
        <label className="field"><span className="field-title">Memory allocation <strong>{formatRam(ram)}</strong></span><input type="range" min="1024" max="16384" step="256" value={Math.min(ram, 16384)} onChange={(e) => setRam(Number(e.target.value))} /><div className="ram-input"><input aria-label="Memory in MB" type="number" min="1024" max="65536" step="256" value={ram} onChange={(e) => setRam(Number(e.target.value))} required /><small>MB · leave memory for Windows and other apps</small></div></label>
        {instance ? <label className="field"><span>Game directory</span><input value={instance.gameDirectory} readOnly /><small>Existing directories stay in place. Create a new instance to choose another location.</small></label> : <PathField label="Instance parent directory" value={parent} onChange={setParent} onError={setError} hint="A unique game folder is created here. Your existing files stay untouched." />}
        <PathField label="Java executable override" kind="java" value={java} onChange={setJava} onError={setError} hint="Optional. Leave blank to use the launcher default or automatic detection." />
      </fieldset>
      <div className="notice"><AlertCircle size={16} /><span>{rocketPack
        ? offline ? 'Go online to download Rocket Pack, or choose another setup.'
          : packUnavailable ? 'Choose a Minecraft release for Rocket Pack, or choose another setup for this snapshot.'
          : 'Rocket Pack downloads compatible mods during creation. Use Install & play on the Launch Pad to finish setup.'
        : 'Install or verify Minecraft files from the Launch Pad after saving.'}</span></div>
      {rocketPack && busy && packProgress && <div className="pack-progress">
        <p role="status">{packProgress[0]} of {packProgress[1]} pack steps complete</p>
        <progress className="game-progress" aria-label="Rocket Pack setup progress" value={packProgress[0]} max={packProgress[1]} />
      </div>}
      {(error || saveError) && <p role="alert" className="form-error">{error || saveError}</p>}
      <div className="modal-actions"><button type="button" className="button secondary" onClick={onClose} disabled={busy}>Cancel</button><button className="button primary" type="submit" disabled={busy || !selectedVersion || packUnavailable || (needsLoader && !loaderVersion)}>{busy && <Spinner />}{instance ? 'Save changes' : rocketPack ? 'Create Rocket Pack instance' : 'Create instance'}</button></div>
    </form>
  </Modal>;
}
