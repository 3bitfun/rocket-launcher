import { useEffect, useRef, useState } from 'react';
import { AnimatePresence, MotionConfig, motion, useReducedMotion } from 'motion/react';
import { AlertCircle, CheckCircle2, Download, Monitor, X } from 'lucide-react';
import { Sidebar } from '../components/layout/Sidebar';
import { InstanceEditor } from '../components/instances/InstanceEditor';
import { InstanceManager } from '../components/instances/InstanceManager';
import { Modal } from '../components/ui/Modal';
import { Spinner } from '../components/ui/Primitives';
import { LaunchPad } from '../pages/LaunchPad/LaunchPad';
import { Instances } from '../pages/Instances/Instances';
import { Downloads } from '../pages/Downloads/Downloads';
import { Mods } from '../pages/Mods/Mods';
import { Multiplayer } from '../pages/Multiplayer/Multiplayer';
import { Settings } from '../pages/Settings/Settings';
import { useLauncher } from '../hooks/useLauncher';
import { useGame } from '../hooks/useGame';
import { isDesktop, native } from '../services/native';
import type { AccountsSummary, Instance, Page } from '../types';

const pageDetails: Record<Page, { title: string; subtitle: string }> = {
  launchpad: { title: 'Launch Pad', subtitle: 'Choose an instance and play.' },
  instances: { title: 'Instances', subtitle: 'Manage your Minecraft versions, loaders, and game folders.' },
  mods: { title: 'Mods', subtitle: 'Find compatible mods or start with Rocket Pack.' },
  multiplayer: { title: 'Multiplayer', subtitle: 'Join saved servers or share a world from your PC.' },
  downloads: { title: 'Downloads', subtitle: 'Track game installs and refresh the version catalogue.' },
  settings: { title: 'Settings', subtitle: 'Manage accounts, game defaults, and launcher updates.' },
};

function accentInk(hex: string) {
  const components = [1, 3, 5].map((offset) => parseInt(hex.slice(offset, offset + 2), 16) / 255).map((value) => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4);
  const luminance = components[0] * 0.2126 + components[1] * 0.7152 + components[2] * 0.0722;
  return luminance > 0.179 ? '#111318' : '#FFFFFF';
}

export default function App() {
  const launcher = useLauncher();
  const { snapshot: stored, catalog, catalogError, fetching, mutate } = launcher;
  const [accounts, setAccounts] = useState<AccountsSummary | null>(null);
  const game = useGame(stored, launcher.setToast, accounts);
  const busy = launcher.busy || game.working || game.download?.stage === 'installing' || game.download?.stage === 'downloading';
  const [page, setPage] = useState<Page>('launchpad');
  const main = useRef<HTMLElement>(null);
  const [editor, setEditor] = useState<Instance | 'new' | null>(null);
  const [managedId, setManagedId] = useState<string | null>(null);
  const [modsTarget, setModsTarget] = useState<string>();
  const [removing, setRemoving] = useState<Instance | null>(null);
  const [settingsDirty, setSettingsDirty] = useState(false);
  const [pendingPage, setPendingPage] = useState<Page | null>(null);
  const systemReduced = useReducedMotion();
  const reduced = Boolean(systemReduced || stored?.settings.reducedMotion === 'always' || stored?.settings.animations === false);
  const snapshot = stored && { ...stored, instances: stored.instances.map((instance) => ({ ...instance, readiness: game.status(instance).readiness })) };
  const activeAccount = accounts?.profiles.find((account) => account.id === accounts.activeId);

  useEffect(() => { main.current?.scrollTo(0, 0); }, [page]);

  useEffect(() => {
    let active = true;
    void native.accounts().then((value) => {
      if (active) setAccounts((current) => current ?? value);
    }).catch(() => {});
    return () => { active = false; };
  }, []);

  useEffect(() => {
    if (!stored) return;
    const root = document.documentElement;
    root.style.setProperty('--accent', stored.settings.accentColor);
    root.style.setProperty('--accent-ink', accentInk(stored.settings.accentColor));
    root.dataset.theme = stored.settings.theme;
    root.dataset.motion = reduced ? 'reduced' : 'full';
  }, [stored, reduced]);

  useEffect(() => {
    if (!settingsDirty) return;
    const warn = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ''; };
    window.addEventListener('beforeunload', warn);
    return () => window.removeEventListener('beforeunload', warn);
  }, [settingsDirty]);

  function navigate(next: Page) {
    if (next === page) return;
    if (settingsDirty) { setPendingPage(next); return; }
    setModsTarget(undefined);
    setPage(next);
  }
  function select(instance: Instance) { if (instance.id !== snapshot?.activeInstanceId) void mutate(() => native.select(instance.id), `${instance.name} is on the launch pad.`); }
  function openEditor(instance: Instance | 'new') { launcher.setToast(null); setEditor(instance); }
  function openRemoval(instance: Instance) { launcher.setToast(null); setRemoving(instance); }
  function openManager(instance: Instance) { launcher.setToast(null); setManagedId(instance.id); }
  function duplicate(instance: Instance) {
    void mutate(() => native.duplicate(instance.id), 'Configuration duplicated into a new, empty instance.');
  }

  if (!isDesktop()) return <div className="startup-screen"><span className="brand-mark"><img src="/rocket.svg" alt="" /></span><h1>Rocket Launcher</h1><p>This is a native desktop application.<br />Open the Tauri app to use real storage and Minecraft versions.</p><code>npm run desktop</code><span className="tag"><Monitor size={14} /> Browser view · native features unavailable</span></div>;
  if (!snapshot) return <div className="startup-screen"><span className="brand-mark"><img src="/rocket.svg" alt="" /></span><h1>Rocket Launcher</h1>{launcher.startupError ? <><p role="alert" className="form-error">{launcher.startupError}</p><button className="button primary" onClick={() => void launcher.load()}>Try again</button></> : <><Spinner size={25} /><p>Preparing your launch pad…</p></>}</div>;
  const selected = snapshot.instances.find((instance) => instance.id === snapshot.activeInstanceId);
  const managed = snapshot.instances.find((instance) => instance.id === managedId);
  const retry = snapshot.instances.find((instance) => instance.id === game.download?.instanceId && instance.version === game.download.version);
  const downloading = game.download?.stage === 'downloading' || game.download?.stage === 'installing';

  return <MotionConfig reducedMotion={reduced ? 'always' : 'user'} transition={{ duration: reduced ? 0 : 0.2 }}>
    <div className="app-shell"><a className="skip-link" href="#main-content">Skip to content</a><Sidebar page={page} onNavigate={navigate} count={snapshot.instances.length} version={snapshot.appVersion} account={activeAccount} offline={accounts?.offline ?? false} />
      <main ref={main} id="main-content" tabIndex={-1}><header className="page-header">
        <div><h1>{pageDetails[page].title}</h1><p>{pageDetails[page].subtitle}</p></div>
        {downloading ? <button className="button secondary small-button" onClick={() => navigate('downloads')}><Download size={15} /> View download</button>
          : accounts?.offline && <span className="tag">Offline mode</span>}
      </header>
        <AnimatePresence mode="wait" initial={false}><motion.div key={page} className="page-content" initial={{ opacity: 0, y: reduced ? 0 : 6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: reduced ? 0 : -4 }} transition={{ duration: reduced ? 0 : 0.16 }}>
          {page === 'launchpad' && <LaunchPad snapshot={snapshot} busy={busy} status={selected && game.status(selected)} onCreate={() => openEditor('new')} onEdit={openEditor} onManage={openManager} onSelect={select} onNavigate={navigate} onInstall={(instance) => void game.run(instance, 'install')} onIgnite={(instance) => void game.run(instance, 'launch')} onCheck={(instance) => void game.check(instance.id)} />}
          {page === 'instances' && <Instances snapshot={snapshot} busy={busy} onCreate={() => openEditor('new')} onEdit={openEditor} onManage={openManager} onDuplicate={duplicate} onDelete={openRemoval} />}
          {page === 'downloads' && <Downloads catalog={catalog} error={catalogError} fetching={fetching} onRefresh={() => void launcher.refreshVersions()} progress={game.download} onRetry={!busy && retry ? () => void game.run(retry, 'install') : undefined} />}
          {page === 'mods' && <Mods snapshot={snapshot} catalog={catalog} offline={accounts?.offline ?? false} busy={busy} initialInstanceId={modsTarget} onPlay={() => navigate('launchpad')} onModpackInstalled={() => void launcher.load()} onInstallPack={(version, progress) => mutate(() => native.installRocketPack(version, progress), 'Rocket Pack instance created.')} />}
          {page === 'multiplayer' && <Multiplayer snapshot={snapshot} offline={accounts?.offline ?? false} busy={busy}
            onManage={openManager} onJoin={(instance, address) => game.run(instance, 'launch', address)} />}
          {page === 'settings' && <Settings snapshot={snapshot} busy={busy} onSave={(settings) => mutate(() => native.settings(settings), 'Preferences saved. Make yourself at home.')} onDirtyChange={setSettingsDirty} onAuthStateChanged={setAccounts} />}
        </motion.div></AnimatePresence>
      </main>
    </div>
    {managed && !editor && !removing && <InstanceManager key={managed.id} instance={managed} status={game.status(managed)}
      active={managed.id === snapshot.activeInstanceId} offline={accounts?.offline ?? false} synced={snapshot.settings.syncRocketClient} busy={busy} notice={launcher.toast}
      onClose={() => setManagedId(null)} onPlay={() => void game.run(managed, 'launch')} onInstall={() => void game.run(managed, 'install')}
      onCheck={() => void game.check(managed.id)} onProfile={() => { setManagedId(null); navigate('settings'); }}
      onActivate={() => select(managed)} onEdit={() => openEditor(managed)}
      onMods={() => { setManagedId(null); navigate('mods'); setModsTarget(managed.id); }}
      onDuplicate={() => duplicate(managed)} onRemove={() => openRemoval(managed)} />}
    {editor && <InstanceEditor instance={editor === 'new' ? undefined : editor} settings={snapshot.settings} catalog={catalog} catalogError={catalogError} saveError={launcher.toast?.kind === 'error' ? launcher.toast.message : null} fetching={fetching} busy={busy} offline={accounts?.offline ?? false} onRefresh={() => void launcher.refreshVersions()} onClose={() => setEditor(null)} onSave={(input, onPackProgress) => mutate(() => {
      if (editor !== 'new') return native.update(editor.id, input);
      return onPackProgress ? native.installRocketPack(input.version, onPackProgress, input) : native.create(input);
    }, editor === 'new' ? 'Instance created. Your next world has a home.' : 'Instance updated.')} />}
    {removing && <Modal title={`Remove ${removing.name}?`} description="This removes the instance from Rocket Launcher." onClose={() => setRemoving(null)} busy={busy}><div className="notice"><AlertCircle size={18} /><span>Your game directory and all its files will be preserved.</span></div><code className="path-preview">{removing.gameDirectory}</code>{launcher.toast?.kind === 'error' && <p role="alert" className="form-error">{launcher.toast.message}</p>}<div className="modal-actions"><button className="button secondary" disabled={busy} onClick={() => setRemoving(null)}>Keep instance</button><button className="button danger" disabled={busy} onClick={async () => { if (await mutate(() => native.remove(removing.id), 'Instance removed. Its files are still on disk.')) setRemoving(null); }}>{busy && <Spinner size={16} />}Remove instance</button></div></Modal>}
    {pendingPage && <Modal title="Discard unsaved preferences?" description="Your changes have not been saved yet." onClose={() => setPendingPage(null)}><div className="modal-actions"><button className="button secondary" onClick={() => setPendingPage(null)}>Keep editing</button><button className="button danger" onClick={() => { setSettingsDirty(false); setPage(pendingPage); setPendingPage(null); }}>Discard changes</button></div></Modal>}
    <AnimatePresence>{launcher.toast && <motion.div className={`toast ${launcher.toast.kind}`} role={launcher.toast.kind === 'error' ? 'alert' : 'status'} initial={{ opacity: 0, y: reduced ? 0 : 12 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }}>
      {launcher.toast.kind === 'error' ? <AlertCircle size={19} /> : <CheckCircle2 size={19} />}<span>{launcher.toast.message}</span><button className="icon-button" aria-label="Dismiss notification" onClick={() => launcher.setToast(null)}><X size={16} /></button>
    </motion.div>}</AnimatePresence>
  </MotionConfig>;
}
