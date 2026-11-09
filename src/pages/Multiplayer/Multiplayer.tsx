import { useEffect, useState } from 'react';
import { Clipboard, Globe2, Play, Plus, Radio, Star, Users } from 'lucide-react';
import { Spinner } from '../../components/ui/Primitives';
import { loaderName } from '../../services/loaders';
import { errorMessage, native } from '../../services/native';
import type { AppSnapshot, Instance, MultiplayerAddon, SavedServer, ServerStatus } from '../../types';

const emptyServer: SavedServer = { id: '', name: '', address: '', instanceId: null, favorite: false, lastJoined: null };
type Feature = 'via' | 'hosting';
const addonNames: Record<string, string> = { viafabric: 'ViaFabric', viafabricplus: 'ViaFabricPlus', e4mc: 'e4mc', e4mc_minecraft: 'e4mc' };

function canQuickPlay(version: string) {
  const match = /^(\d+)\.(\d+)(?:\.\d+)?(?:$|[-+])/.exec(version);
  return !!match && (+match[1] >= 26 || (+match[1] === 1 && +match[2] >= 20));
}

export function Multiplayer({ snapshot, offline, busy, onManage, onJoin }: {
  snapshot: AppSnapshot; offline: boolean; busy: boolean;
  onManage: (instance: Instance) => void; onJoin: (instance: Instance, address: string) => Promise<boolean>;
}) {
  const [servers, setServers] = useState<SavedServer[] | null>(null);
  const [draft, setDraft] = useState<SavedServer>(emptyServer);
  const [defaultInstance, setDefaultInstance] = useState(snapshot.activeInstanceId ?? '');
  const [address, setAddress] = useState('');
  const [status, setStatus] = useState<Record<string, ServerStatus | string>>({});
  const [checking, setChecking] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [reload, setReload] = useState(0);
  const [modReload, setModReload] = useState(0);
  const [addons, setAddons] = useState<MultiplayerAddon[] | null>(null);
  const [addonError, setAddonError] = useState('');
  const [installing, setInstalling] = useState<Feature | null>(null);
  const instances = snapshot.instances;
  const selected = instances.find((instance) => instance.id === defaultInstance)
    ?? instances.find((instance) => instance.id === snapshot.activeInstanceId) ?? instances[0];
  const fabricId = selected?.loader === 'fabric' ? selected.id : undefined;
  const sorted = [...(servers ?? [])].sort((a, b) =>
    Number(b.favorite) - Number(a.favorite) || (b.lastJoined ?? 0) - (a.lastJoined ?? 0) || a.name.localeCompare(b.name));

  useEffect(() => {
    let active = true;
    void native.savedServers().then((value) => { if (active) setServers(value); })
      .catch((reason) => { if (active) setError(errorMessage(reason)); });
    return () => { active = false; };
  }, [reload]);

  useEffect(() => {
    let active = true;
    setAddons(null);
    setAddonError('');
    if (fabricId) void native.multiplayerAddons(fabricId).then((value) => { if (active) setAddons(value); })
      .catch((reason) => { if (active) setAddonError(errorMessage(reason)); });
    return () => { active = false; };
  }, [fabricId, modReload]);

  function instanceFor(server: SavedServer) {
    const pinned = instances.find((instance) => instance.id === server.instanceId);
    const reported = status[server.address];
    const version = typeof reported === 'object' ? reported.version.match(/\b(?:1|2[6-9])\.\d+(?:\.\d+)?\b/)?.[0] : undefined;
    return pinned ?? instances.find((instance) => instance.id === selected?.id && instance.version === version)
      ?? instances.find((instance) => instance.version === version) ?? selected;
  }

  function canJoin(instance: Instance | undefined) {
    return !!instance && canQuickPlay(instance.version)
      && (instance.readiness === 'ready' || instance.readiness === 'missing_files')
      && !offline && !busy && !pending && !installing;
  }

  async function changeServers(action: () => Promise<SavedServer[]>, clear = false) {
    setPending(true); setError(''); setNotice('');
    try {
      setServers(await action());
      if (clear) setDraft(emptyServer);
    } catch (reason) { setError(errorMessage(reason)); }
    finally { setPending(false); }
  }

  async function join(instance: Instance | undefined, host: string, server?: SavedServer) {
    if (!instance || !canJoin(instance)) return;
    setError(''); setNotice('');
    if (await onJoin(instance, host) && server) {
      const pinned = instances.some((item) => item.id === server.instanceId) ? server.instanceId : null;
      await changeServers(() => native.saveServer({ ...server, instanceId: pinned, lastJoined: Math.floor(Date.now() / 1000) }));
    }
  }

  async function ping(host: string) {
    setChecking(host);
    try {
      const result = await native.pingServer(host);
      setStatus((value) => ({ ...value, [host]: result }));
    } catch (reason) { setStatus((value) => ({ ...value, [host]: errorMessage(reason) })); }
    finally { setChecking(null); }
  }

  async function copy(host: string) {
    setNotice(''); setError('');
    try { await navigator.clipboard.writeText(host); setNotice('Server address copied.'); }
    catch { setError('Could not access the clipboard. Select and copy the address shown on screen.'); }
  }

  async function install(feature: Feature) {
    if (!fabricId) return;
    setInstalling(feature); setAddonError(''); setNotice('');
    try {
      await native.installMultiplayerAddon(fabricId, feature);
      setNotice(feature === 'hosting' ? 'e4mc installed. Launch this instance and open a singleplayer world.' : 'Protocol compatibility add-on installed.');
      setModReload((value) => value + 1);
    } catch (reason) { setAddonError(errorMessage(reason)); }
    finally { setInstalling(null); }
  }

  const disabled = busy || pending || !!installing;
  const inGame = selected?.readiness === 'running' || selected?.readiness === 'launching';
  return <div className="multiplayer-page">
    <section className="panel settings-section">
      <div className="settings-section-title"><Users size={20} /><div><h2>Connect to a world</h2><p>Paste an e4mc address or any Java server hostname.</p></div></div>
      <div className="multiplayer-fields">
        <label className="field"><span>Instance</span><select disabled={disabled || instances.length === 0} value={selected?.id ?? ''} onChange={(e) => setDefaultInstance(e.target.value)}>
          {!instances.length && <option value="">Create an instance first</option>}
          {instances.map((instance) => <option key={instance.id} value={instance.id}>{instance.name} · {instance.version} · {loaderName(instance.loader)}</option>)}
        </select></label>
        <label className="field"><span>Server address</span><input value={address} onChange={(e) => setAddress(e.target.value)} maxLength={260} placeholder="example.net" /></label>
        <button className="button primary" type="button" disabled={!address.trim() || !canJoin(selected)} onClick={() => void join(selected, address)}><Play size={16} /> Join</button>
      </div>
      <p className="panel-footnote">{offline ? 'Turn off Offline Mode to check status or join from Rocket.'
        : selected && !canQuickPlay(selected.version) ? 'For this Minecraft version, launch the game and paste the address in Multiplayer → Direct Connection.'
        : 'Quick Play opens Minecraft directly to the server. Servers may require Microsoft sign-in and a matching modpack.'}</p>
      {selected && <button type="button" className="text-button" onClick={() => onManage(selected)}>Manage {selected.name}</button>}
    </section>

    <section className="panel settings-section">
      <div className="settings-section-title"><Globe2 size={20} /><div><h2>Saved servers</h2><p>Favorites stay at the top, followed by recent joins.</p></div></div>
      <form className="multiplayer-fields" onSubmit={(event) => { event.preventDefault(); void changeServers(() => native.saveServer(draft), true); }}>
        <label className="field"><span>Name</span><input required maxLength={64} value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} /></label>
        <label className="field"><span>Address</span><input required maxLength={260} value={draft.address} onChange={(e) => setDraft({ ...draft, address: e.target.value })} /></label>
        <label className="field"><span>Preferred instance</span><select value={draft.instanceId ?? ''} onChange={(e) => setDraft({ ...draft, instanceId: e.target.value || null })}>
          <option value="">Suggest an instance</option>
          {instances.map((instance) => <option key={instance.id} value={instance.id}>{instance.name}</option>)}
        </select></label>
        <button type="submit" className="button secondary" disabled={disabled || !servers}><Plus size={16} /> {draft.id ? 'Save changes' : 'Add server'}</button>
        {draft.id && <button type="button" className="button secondary" onClick={() => setDraft(emptyServer)}>Cancel</button>}
      </form>
      {!servers && !error && <div className="loading-inline"><Spinner /> Loading saved servers…</div>}
      {!servers && error && <button className="button secondary small-button" type="button" onClick={() => { setError(''); setReload((value) => value + 1); }}>Retry</button>}
      {servers?.length === 0 && <p className="panel-footnote">Add a server or shared world address to get started.</p>}
      {sorted.map((server) => {
        const result = status[server.address];
        const instance = instanceFor(server);
        return <article key={server.id} className="multiplayer-server">
          <button type="button" className="icon-button" disabled={disabled} aria-label={server.favorite ? `Unfavorite ${server.name}` : `Favorite ${server.name}`} aria-pressed={server.favorite}
            onClick={() => void changeServers(() => native.saveServer({ ...server, instanceId: instances.some((item) => item.id === server.instanceId) ? server.instanceId : null, favorite: !server.favorite }))}><Star size={18} fill={server.favorite ? 'currentColor' : 'none'} /></button>
          <div className="multiplayer-server-info">
            <strong>{server.name}</strong><code>{server.address}</code>
            {typeof result === 'object' && <div className="modrinth-tags"><span className="tag">{result.version}</span><span className="tag">{result.players}/{result.maxPlayers} players</span><span className="tag">{result.latencyMs} ms</span></div>}
            {typeof result === 'string' && <p className="form-error">{result}</p>}
            {instance && <small>{server.instanceId === instance.id ? 'Preferred' : 'Suggested'} instance · {instance.name} ({instance.version})</small>}
          </div>
          <div className="inline-actions multiplayer-server-actions">
            <button type="button" className="button secondary small-button" onClick={() => void ping(server.address)} disabled={offline || !!checking}>{checking === server.address ? <Spinner size={14} /> : <Radio size={14} />} Status</button>
            <button type="button" className="button secondary small-button" onClick={() => void copy(server.address)}><Clipboard size={14} /> Copy</button>
            <button type="button" className="button primary small-button" onClick={() => void join(instance, server.address, server)} disabled={!canJoin(instance)} title={instance && !canQuickPlay(instance.version) ? 'Copy the address and use Direct Connection' : undefined}><Play size={14} /> Join</button>
            <button type="button" className="text-button" disabled={disabled} onClick={() => setDraft({ ...server, instanceId: instances.some((item) => item.id === server.instanceId) ? server.instanceId : null })}>Edit</button>
            <button type="button" className="text-button" disabled={disabled} onClick={() => void changeServers(() => native.removeSavedServer(server.id), draft.id === server.id)}>Remove</button>
          </div>
        </article>;
      })}
    </section>

    <section className="panel settings-section">
      <div className="settings-section-title"><Radio size={20} /><div><h2>Multiplayer add-ons</h2><p>{selected ? `Setup for ${selected.name} · Minecraft ${selected.version}` : 'Create a Fabric instance to get started.'}</p></div></div>
      {!fabricId ? <p>Select a Fabric instance above to add world sharing or protocol compatibility.</p> : <>
        {(['via', 'hosting'] as const).map((feature) => {
          const matches = addons?.filter((addon) => feature === 'via' ? addon.id.startsWith('viafabric') : addon.id.startsWith('e4mc'));
          const active = matches?.find((addon) => addon.enabled);
          return <div className="setting-row" key={feature}>
            <div><strong>{feature === 'via' ? 'Server version compatibility' : 'Share your world'}</strong>
              <small>{feature === 'via' ? 'Installs ViaFabricPlus or ViaFabric when a matching build is available. Gameplay mods still need to match the server.'
                : 'Installs e4mc. Open a singleplayer world and use Share World in the Rocket Client pause menu, or Open to LAN. Anyone with the address can attempt to join.'}</small>
              {matches && matches.filter((addon) => addon.enabled).length > 1 && <small className="form-error">Multiple copies are enabled. Disable extras in Installed mods.</small>}
            </div>
            {active ? <span className="tag">{addonNames[active.id]} enabled</span> : matches?.length ? <span className="tag">Enable in Installed mods</span>
              : <button type="button" className="button secondary small-button" disabled={disabled || offline || inGame || !addons} onClick={() => void install(feature)}>
                {installing === feature && <Spinner size={14} />}{feature === 'via' ? 'Install ViaFabric' : 'Install e4mc'}
              </button>}
          </div>;
        })}
        {!addons && !addonError && <p className="panel-footnote">Checking installed add-ons…</p>}
        {inGame && <p className="panel-footnote">Close Minecraft before installing add-ons.</p>}
        {addonError && <div><p className="form-error" role="alert">{addonError}</p><button type="button" className="text-button" onClick={() => setModReload((value) => value + 1)}>Refresh add-ons</button></div>}
      </>}
    </section>
    {error && <p className="form-error" role="alert">{error}</p>}
    {notice && <p className="notice" role="status">{notice}</p>}
  </div>;
}
