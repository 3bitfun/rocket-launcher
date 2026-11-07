import { useEffect, useState, type FormEvent } from 'react';
import { Check, Cpu, Folder, Info, LogIn, Palette, Save, Settings2, UserRound } from 'lucide-react';
import type { AccountsSummary, AppSnapshot, Settings as SettingsModel } from '../../types';
import { native, errorMessage } from '../../services/native';
import { Modal } from '../../components/ui/Modal';
import { PathField, Spinner, Toggle } from '../../components/ui/Primitives';
import { LauncherUpdates } from './LauncherUpdates';

const accents = ['#FF704D', '#E4B75E', '#66BD9E', '#78A5EF', '#B19AE8', '#E68DA8'];

export function Settings({ snapshot, busy, onSave, onDirtyChange, onAuthStateChanged }: { snapshot: AppSnapshot; busy: boolean; onSave: (settings: SettingsModel) => Promise<boolean>; onDirtyChange: (dirty: boolean) => void; onAuthStateChanged?: (accounts: AccountsSummary) => void }) {
  const [draft, setDraft] = useState(snapshot.settings);
  const [error, setError] = useState<string | null>(null);
  const [accounts, setAccounts] = useState<AccountsSummary | null>(null);
  const [signingIn, setSigningIn] = useState(false);
  const [accountBusy, setAccountBusy] = useState(false);
  const [offlineName, setOfflineName] = useState('');
  const [changingOffline, setChangingOffline] = useState(false);
  const [logs, setLogs] = useState<string | null>(null);
  const [loadingLogs, setLoadingLogs] = useState(false);
  const [installingUpdate, setInstallingUpdate] = useState(false);
  useEffect(() => {
    let active = true;
    void native.accounts().then((value) => { if (active) setAccounts(value); })
      .catch((reason) => { if (active) setError(errorMessage(reason)); });
    return () => { active = false; };
  }, []);
  const dirty = JSON.stringify(draft) !== JSON.stringify(snapshot.settings);
  const activeAccount = accounts?.profiles.find((profile) => profile.id === accounts.activeId);
  function applyAccounts(value: AccountsSummary) {
    setAccounts(value);
    onAuthStateChanged?.(value);
  }
  function update<K extends keyof SettingsModel>(key: K, value: SettingsModel[K]) {
    const next = { ...draft, [key]: value };
    setDraft(next); onDirtyChange(JSON.stringify(next) !== JSON.stringify(snapshot.settings));
  }
  async function save(event: FormEvent) {
    event.preventDefault(); setError(null);
    if (await onSave(draft)) onDirtyChange(false);
  }
  async function showLogs() {
    setLoadingLogs(true);
    try { setLogs(await native.logs()); }
    catch (reason) { setError(errorMessage(reason)); }
    finally { setLoadingLogs(false); }
  }
  async function signIn() {
    setSigningIn(true);
    setError(null);
    try { applyAccounts(await native.signIn()); }
    catch (reason) { setError(errorMessage(reason)); }
    finally { setSigningIn(false); }
  }
  async function selectAccount(id: string) {
    setAccountBusy(true);
    setError(null);
    try { applyAccounts(await native.selectAccount(id)); }
    catch (reason) { setError(errorMessage(reason)); }
    finally { setAccountBusy(false); }
  }
  async function createLocalTestAccount() {
    setAccountBusy(true);
    setError(null);
    try { applyAccounts(await native.createTestAccount()); }
    catch (reason) { setError(errorMessage(reason)); }
    finally { setAccountBusy(false); }
  }
  async function createOfflineAccount() {
    setAccountBusy(true);
    setError(null);
    try {
      applyAccounts(await native.createOfflineAccount(offlineName));
      setOfflineName('');
    } catch (reason) { setError(errorMessage(reason)); }
    finally { setAccountBusy(false); }
  }
  async function signOut(id: string) {
    setAccountBusy(true);
    setError(null);
    try { applyAccounts(await native.signOut(id)); }
    catch (reason) { setError(errorMessage(reason)); }
    finally { setAccountBusy(false); }
  }
  async function setOfflineMode(offline: boolean) {
    if (!accounts) return;
    setChangingOffline(true);
    setError(null);
    try { applyAccounts(await native.setOfflineMode(offline)); }
    catch (reason) { setError(errorMessage(reason)); }
    finally { setChangingOffline(false); }
  }

  return <form className="settings-form" onSubmit={(event) => void save(event)}>
    <div className="page-intro"><div><h2>A launcher that feels like you.</h2><p>The small details, just how you like them.</p></div><span className="tag">v{snapshot.appVersion}</span></div>
    <fieldset disabled={busy || installingUpdate} className="settings-fields">
      <section className="panel settings-section"><div className="settings-section-title"><Palette size={19} /><div><h2>Appearance</h2><p>Set the mood for your launch pad.</p></div></div>
        <div className="setting-row"><div><strong>Theme</strong><small>Choose your preferred light level.</small></div><div className="segmented" role="group" aria-label="Theme">{(['dark', 'light', 'system'] as const).map((theme) => <button type="button" key={theme} aria-pressed={draft.theme === theme} className={draft.theme === theme ? 'active' : ''} onClick={() => update('theme', theme)}>{theme[0].toUpperCase() + theme.slice(1)}</button>)}</div></div>
        <div className="setting-row"><div><strong>Accent colour</strong><small>A personal touch. Rocket coral by default.</small></div><div className="accent-options">{accents.map((accent) => <button type="button" key={accent} style={{ background: accent }} aria-label={`Accent ${accent}`} aria-pressed={draft.accentColor.toUpperCase() === accent} onClick={() => update('accentColor', accent)}>{draft.accentColor.toUpperCase() === accent && <Check size={16} />}</button>)}<label className="custom-colour" title="Choose a custom accent colour"><input type="color" aria-label="Custom accent colour" value={draft.accentColor} onChange={(e) => update('accentColor', e.target.value)} /><span>+</span></label></div></div>
        <Toggle label="Decorative animations" description="Gentle transitions and a little extra personality." checked={draft.animations} onChange={(value) => update('animations', value)} />
        <div className="setting-row"><div><strong>Reduced motion</strong><small>Your system preference is always respected.</small></div><select aria-label="Reduced motion" value={draft.reducedMotion} onChange={(e) => update('reducedMotion', e.target.value as SettingsModel['reducedMotion'])}><option value="system">Follow system</option><option value="always">Always reduce</option></select></div>
      </section>

      <section className="panel settings-section"><div className="settings-section-title"><Cpu size={19} /><div><h2>Minecraft</h2><p>Defaults for the worlds you create next.</p></div></div>
        <div className="setting-row"><div><strong>Default memory</strong><small>Existing instances keep their own allocation.</small></div><label className="number-unit"><input type="number" aria-label="Default memory in MB" min="1024" max="65536" step="256" value={draft.defaultRamMb} onChange={(e) => update('defaultRamMb', Number(e.target.value))} required /><span>MB</span></label></div>
        <div className="settings-paths"><PathField label="Default Java executable" kind="java" value={draft.javaPath ?? ''} onChange={(value) => update('javaPath', value.trim() || null)} onError={setError} hint="Optional. Leave blank to search Java installations automatically." /><PathField label="Minecraft instance directory" value={draft.minecraftDirectory} onChange={(value) => update('minecraftDirectory', value)} onError={setError} hint="New instances get a unique folder here. This does not move existing instances." /></div>
        <Toggle label="Show snapshots" description="Include experimental Minecraft versions by default." checked={draft.showSnapshots} onChange={(value) => update('showSnapshots', value)} />
      </section>

      <section className="panel settings-section"><div className="settings-section-title"><Settings2 size={19} /><div><h2>Launcher</h2><p>Everything has its place.</p></div></div>
        {accounts && <LauncherUpdates offline={accounts.offline} blocked={dirty || signingIn || accountBusy || changingOffline} onInstallingChange={setInstallingUpdate} />}
        <div className="setting-row"><div><strong>Network request timeout</strong><small>Applies to catalogue requests. Failed requests retry once.</small></div><label className="number-unit"><input aria-label="Network request timeout in seconds" type="number" min="5" max="120" value={draft.requestTimeoutSeconds} onChange={(e) => update('requestTimeoutSeconds', Number(e.target.value))} required /><span>sec</span></label></div>
        <div className="stored-path"><Folder size={16} /><div><strong>Shared cache</strong><code>{snapshot.paths.cache}</code></div></div>
        <div className="stored-path"><Folder size={16} /><div><strong>Configuration</strong><code>{snapshot.paths.config}</code></div></div>
        <div className="setting-row"><div><strong>Launcher logs</strong><small>Local structured logs. Keeps the seven most recent daily files.</small></div><button type="button" className="button secondary small-button" onClick={() => void showLogs()} disabled={loadingLogs}>{loadingLogs && <Spinner size={14} />}View logs</button></div>
      </section>

      <section className="panel settings-section"><div className="settings-section-title"><UserRound size={19} /><div><h2>Accounts</h2><p>Choose an offline profile or sign in with Microsoft.</p></div></div>
        <div className="account-state"><span className="avatar"><UserRound size={22} /></span><div><strong>{activeAccount?.type === 'offline' ? `Offline · ${activeAccount.name}` : activeAccount?.type === 'local_test' ? 'Local Test — Not Authenticated' : activeAccount ? `Connected as ${activeAccount.name}` : 'No Minecraft account connected'}</strong><p>{activeAccount?.type === 'offline' ? 'Ready for local play. Online Minecraft services require Microsoft sign-in.' : activeAccount?.type === 'local_test' ? 'Developer profile for testing account UI, instance management, and launch preparation only.' : activeAccount ? accounts?.offline ? 'Offline mode is active. Turn it off to sign in or switch accounts.' : 'Ownership was verified. Sign in again to switch accounts.' : 'Create an offline profile or sign in with Microsoft.'}</p></div>
          {accounts && accounts.profiles.length > 1 && <select aria-label="Active account" value={accounts.activeId ?? ''} disabled={accountBusy || signingIn || changingOffline} onChange={(event) => void selectAccount(event.target.value)}>{accounts.profiles.map((profile) => <option key={profile.id} value={profile.id}>{profile.type === 'local_test' ? `Local Test · ${profile.name}` : profile.type === 'offline' ? `Offline · ${profile.name}` : `Microsoft · ${profile.name}`}</option>)}</select>}
          <button type="button" className="button secondary small-button" onClick={() => void signIn()} disabled={busy || signingIn || accountBusy || Boolean(accounts?.offline)}>{signingIn ? <Spinner size={15} /> : <LogIn size={15} />}{activeAccount ? 'Sign in / switch' : 'Sign in'}</button>
        </div>
        <div className="setting-row"><label className="field"><span>Offline username</span><input value={offlineName} onChange={(event) => setOfflineName(event.target.value)} maxLength={16} placeholder="Your Minecraft name" /></label><button type="button" className="button secondary small-button" onClick={() => void createOfflineAccount()} disabled={busy || accountBusy || signingIn || !/^[A-Za-z0-9_]{3,16}$/.test(offlineName)}>{accountBusy ? <Spinner size={15} /> : <UserRound size={15} />}Use offline username</button></div>
        <div className="notice"><Info size={16} /><span>Sign-in needs an approved Microsoft app registration configured as ROCKET_MICROSOFT_CLIENT_ID. Tokens are stored in Windows Credential Manager.</span></div>
        {import.meta.env.DEV && <div className="inline-actions"><button type="button" className="button secondary small-button" onClick={() => void createLocalTestAccount()} disabled={busy || accountBusy || signingIn}>{accountBusy ? <Spinner size={15} /> : <UserRound size={15} />}Create Test Account</button></div>}
        {activeAccount && activeAccount.type !== 'local_test' && <button type="button" className="button secondary small-button" disabled={accountBusy || signingIn || changingOffline} onClick={() => void signOut(activeAccount.id)}>{activeAccount.type === 'offline' ? 'Remove offline profile' : 'Sign out'}</button>}
        <Toggle label="Offline mode" description="Pause launcher downloads and use your selected profile without contacting Microsoft." checked={accounts?.offline ?? false} disabled={!accounts || changingOffline || accountBusy || !activeAccount || activeAccount.type === 'local_test'} onChange={(value) => void setOfflineMode(value)} />
      </section>
    </fieldset>
    {error && <p role="alert" className="form-error">{error}</p>}
    <div className="settings-save"><span>{dirty ? 'You have unsaved changes' : 'All preferences saved locally'}</span><div className="inline-actions"><button type="button" className="button secondary" disabled={!dirty || busy} onClick={() => { setDraft(snapshot.settings); onDirtyChange(false); setError(null); }}>Revert</button><button type="submit" className="button primary" disabled={!dirty || busy}>{busy ? <Spinner size={16} /> : <Save size={16} />}Save preferences</button></div></div>
    {logs !== null && <Modal title="Launcher logs" description="Most recent log file · last 64 KB · stored only on this computer" onClose={() => setLogs(null)} wide><pre className="log-view">{logs || 'No log entries yet.'}</pre><p className="panel-footnote">Log directory: {snapshot.paths.logs}</p></Modal>}
  </form>;
}
