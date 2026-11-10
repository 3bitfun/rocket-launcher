import { useEffect, useRef, useState } from 'react';
import { AnimatePresence, MotionConfig, motion, useReducedMotion } from 'motion/react';
import { Activity, AlertCircle, ArrowRight, CheckCircle2, Download, Monitor, Search, X } from 'lucide-react';
import { Sidebar } from '../components/layout/Sidebar';
import { QuickSearch } from '../components/layout/QuickSearch';
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
import { isGameActive, readinessLabel } from '../services/readiness';
import { rocketInstancesByVersion } from '../services/loaders';
import type { AccountsSummary, Instance, Page } from '../types';

const pageDetails: Record<Page, { title: string; subtitle: string }> = {
  launchpad: { title: 'Launch Pad', subtitle: 'Choose an instance and play.' },
  instances: { title: 'Instances', subtitle: 'Manage your Minecraft versions, loaders, and game folders.' },
  mods: { title: 'Mods', subtitle: 'Find compatible mods or start with Rocket Pack.' },
  multiplayer: { title: 'Multiplayer', subtitle: 'Join saved servers or share a world from your PC.' },
  downloads: { title: 'Downloads', subtitle: 'Track game installs and refresh the version catalogue.' },
  settings: { title: 'Settings', subtitle: 'Manage accounts, game defaults, and launcher updates.' },
};
const searchPages = (Object.keys(pageDetails) as Page[]).map((id) => ({ id, title: pageDetails[id].title }));

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
  const [rocketSelection, setRocketSelection] = useState<{ version: string; instanceId: string | null } | null>(null);
  const [packProgress, setPackProgress] = useState<[number, number] | null>(null);
  const [fabricVersions, setFabricVersions] = useState<string[] | null>(null);
  const [packCatalogueError, setPackCatalogueError] = useState(false);
  const [packCatalogueRetry, setPackCatalogueRetry] = useState(0);
  const main = useRef<HTMLElement>(null);
  const [editor, setEditor] = useState<Instance | 'new' | null>(null);
  const [managedId, setManagedId] = useState<string | null>(null);
  const [managerDiagnostics, setManagerDiagnostics] = useState(false);
  const [modsTarget, setModsTarget] = useState<string>();
  const [removing, setRemoving] = useState<Instance | null>(null);
  const [settingsDirty, setSettingsDirty] = useState(false);
  const [pendingPage, setPendingPage] = useState<Page | null>(null);
  const [quickSearch, setQuickSearch] = useState(false);
  const [sessionsOpen, setSessionsOpen] = useState(false);
  const systemReduced = useReducedMotion();
  const reduced = Boolean(systemReduced || stored?.settings.reducedMotion === 'always' || stored?.settings.animations === false);
  const snapshot = stored && { ...stored, instances: stored.instances.map((instance) => ({ ...instance, readiness: game.status(instance).readiness })) };
  const activeAccount = accounts?.profiles.find((account) => account.id === accounts.activeId);
  const browsingRocketVersions = page === 'launchpad' && rocketSelection !== null && accounts?.offline === false;

  useEffect(() => {
    if (!browsingRocketVersions || fabricVersions) return;
    let active = true;
    setPackCatalogueError(false);
    void native.fabricGameVersions()
      .then((versions) => { if (active) setFabricVersions(versions); })
      .catch(() => { if (active) setPackCatalogueError(true); });
    return () => { active = false; };
  }, [browsingRocketVersions, fabricVersions, packCatalogueRetry]);

  useEffect(() => { main.current?.scrollTo(0, 0); }, [page]);

  const dialogOpen = Boolean(quickSearch || sessionsOpen || editor || managedId || removing || pendingPage);
  useEffect(() => {
    if (!stored || dialogOpen) return;
    function openSearch(event: KeyboardEvent) {
      if (event.isComposing || event.altKey || event.shiftKey || !(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== 'k') return;
      event.preventDefault();
      setQuickSearch(true);
    }
    window.addEventListener('keydown', openSearch);
    return () => window.removeEventListener('keydown', openSearch);
  }, [stored, dialogOpen]);

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
  function select(instance: Instance) {
    setRocketSelection(null);
    if (instance.id !== snapshot?.activeInstanceId) {
      void mutate(() => native.select(instance.id), `${instance.name} is on the launch pad.`);
    }
  }
  function switchRocketVersion(version: string) {
    if (!rocketSelection || busy) return;
    const installed = rocketInstancesByVersion(snapshot?.instances ?? []).get(version);
    if (!installed || installed.id === snapshot?.activeInstanceId) {
      setRocketSelection({ version, instanceId: installed?.id ?? null });
      return;
    }
    void mutate(async () => {
      const next = await native.select(installed.id);
      setRocketSelection({ version, instanceId: installed.id });
      return next;
    }, `Rocket Pack ${version} is on the launch pad.`);
  }
  async function playNewRocketPack() {
    if (!rocketSelection?.version || rocketSelection.instanceId || busy || accounts?.offline) return;
    const { version } = rocketSelection;
    let instance: Instance | undefined;
    setPackProgress([0, 0]);
    try {
      const saved = await mutate(async () => {
        const next = await native.installRocketPack(version, setPackProgress);
        instance = next.instances.find((item) => item.id === next.activeInstanceId);
        if (!instance) throw new Error('Rocket Pack was created but could not be selected.');
        setRocketSelection({ version, instanceId: instance.id });
        return next;
      }, `Rocket Pack ${version} is on the launch pad.`);
      if (saved && instance) await game.run(instance, 'first-launch');
    } finally {
      setPackProgress(null);
    }
  }
  function openEditor(instance: Instance | 'new') { launcher.setToast(null); setEditor(instance); }
  function openRemoval(instance: Instance) { launcher.setToast(null); setRemoving(instance); }
  function openManager(instance: Instance, diagnostics = false) {
    launcher.setToast(null);
    setManagerDiagnostics(diagnostics);
    setManagedId(instance.id);
  }
  function duplicate(instance: Instance) {
    void mutate(() => native.duplicate(instance.id), 'Configuration duplicated into a new, empty instance.');
  }
  function toggleFavorite(instance: Instance) {
    const favorite = !instance.favorite;
    void mutate(() => native.favorite(instance.id, favorite), favorite ? 'Added to favorites.' : 'Removed from favorites.');
  }
  function play(instance: Instance) {
    void game.run(instance, instance.rocketPack && instance.readiness === 'requires_download' ? 'first-launch' : 'launch');
  }

  if (!isDesktop()) return <div className="startup-screen"><span className="brand-mark"><img src="/rocket.svg" alt="" /></span><h1>Rocket Launcher</h1><p>This is a native desktop application.<br />Open the Tauri app to use real storage and Minecraft versions.</p><code>npm run desktop</code><span className="tag"><Monitor size={14} /> Browser view · native features unavailable</span></div>;
  if (!snapshot) return <div className="startup-screen"><span className="brand-mark"><img src="/rocket.svg" alt="" /></span><h1>Rocket Launcher</h1>{launcher.startupError ? <><p role="alert" className="form-error">{launcher.startupError}</p><button className="button primary" onClick={() => void launcher.load()}>Try again</button></> : <><Spinner size={25} /><p>Preparing your launch pad…</p></>}</div>;
  const selected = snapshot.instances.find((instance) => instance.id === snapshot.activeInstanceId);
  const rocketMode = rocketSelection?.instanceId === null ? 'choose'
    : rocketSelection && rocketSelection.instanceId === selected?.id ? 'selected' : undefined;
  const managed = snapshot.instances.find((instance) => instance.id === managedId);
  const retry = snapshot.instances.find((instance) => instance.id === game.download?.instanceId && instance.version === game.download.version);
  const downloading = game.download?.stage === 'downloading' || game.download?.stage === 'installing';
  const activeGames = snapshot.instances.filter((instance) => isGameActive(instance.readiness));
  const percent = downloading && game.download?.totalBytes
    ? Math.min(100, Math.floor(game.download.verifiedBytes / game.download.totalBytes * 100))
    : null;

  return <MotionConfig reducedMotion={reduced ? 'always' : 'user'} transition={{ duration: reduced ? 0 : 0.2 }}>
    <div className="app-shell"><a className="skip-link" href="#main-content">Skip to content</a><Sidebar page={page} onNavigate={navigate} count={snapshot.instances.length} version={snapshot.appVersion} account={activeAccount} offline={accounts?.offline ?? false} />
      <main ref={main} id="main-content" tabIndex={-1}><header className="page-header">
        <div className="page-header-title"><span className="page-section-mark" aria-hidden="true" /><div><h1>{pageDetails[page].title}</h1><p>{pageDetails[page].subtitle}</p></div></div>
        <div className="header-actions">
          {downloading ? <button className="button secondary small-button" onClick={() => navigate('downloads')}><Download size={15} />{percent === null ? 'View download' : `Download · ${percent}%`}</button>
            : accounts?.offline && <span className="tag">Offline mode</span>}
          {activeGames.length > 0 && <button className="button secondary small-button" aria-haspopup="dialog" onClick={() => {
            if (activeGames.length === 1) openManager(activeGames[0]);
            else setSessionsOpen(true);
          }}><Activity size={15} />{activeGames.length === 1 ? readinessLabel[activeGames[0].readiness] : `Sessions · ${activeGames.length}`}</button>}
          <button className="button secondary small-button quick-search-button" aria-haspopup="dialog" aria-keyshortcuts="Control+K Meta+K" onClick={() => setQuickSearch(true)}><Search size={15} /> Search <kbd aria-hidden="true">Ctrl K</kbd></button>
        </div>
      </header>
        <motion.div key={page} className="page-content" initial={reduced ? false : { opacity: 0.45, y: 9 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: reduced ? 0 : 0.28, ease: [0.18, 0.72, 0.28, 1] }}>
          {page === 'launchpad' && <LaunchPad snapshot={snapshot} catalog={catalog} busy={busy}
            rocketMode={rocketMode} rocketVersion={rocketSelection?.version} packProgress={packProgress} offline={accounts?.offline ?? false}
            fabricVersions={fabricVersions} packCatalogueError={packCatalogueError} onRetryPackCatalogue={() => setPackCatalogueRetry((attempt) => attempt + 1)}
            onChooseRocket={() => setRocketSelection(selected?.rocketPack ? { version: selected.version, instanceId: selected.id } : { version: '', instanceId: null })} onSwitchVersion={switchRocketVersion} onPlayNewRocket={() => void playNewRocketPack()}
            status={selected && game.status(selected)} onCreate={() => openEditor('new')} onEdit={openEditor} onManage={openManager}
            onSelect={select} onNavigate={navigate} onInstall={(instance) => void game.run(instance, 'install')}
            onIgnite={play} onFavorite={toggleFavorite} onDiagnose={(instance) => openManager(instance, true)}
            stopping={Boolean(selected && game.stopping.has(selected.id))} onAbort={(instance) => void game.abort(instance)} onCheck={(instance) => void game.check(instance.id)} />}
          {page === 'instances' && <Instances snapshot={snapshot} busy={busy} onCreate={() => openEditor('new')} onEdit={openEditor} onManage={openManager} onDuplicate={duplicate} onDelete={openRemoval} onFavorite={toggleFavorite} />}
          {page === 'downloads' && <Downloads catalog={catalog} error={catalogError} fetching={fetching} onRefresh={() => void launcher.refreshVersions()} progress={game.download} onRetry={!busy && retry ? () => void game.run(retry, 'install') : undefined} />}
          {page === 'mods' && <Mods snapshot={snapshot} catalog={catalog} offline={accounts?.offline ?? false} busy={busy} initialInstanceId={modsTarget} onPlay={() => navigate('launchpad')} onModpackInstalled={() => void launcher.load()} onInstallPack={(version, progress) => mutate(() => native.installRocketPack(version, progress), 'Rocket Pack instance created.')} />}
          {page === 'multiplayer' && <Multiplayer snapshot={snapshot} offline={accounts?.offline ?? false} busy={busy}
            onManage={openManager} onJoin={(instance, address) => game.run(instance, 'launch', address)} />}
          {page === 'settings' && <Settings snapshot={snapshot} busy={busy} onSave={(settings) => mutate(() => native.settings(settings), 'Preferences saved. Make yourself at home.')} onDirtyChange={setSettingsDirty} onAuthStateChanged={setAccounts} />}
        </motion.div>
      </main>
    </div>
    {quickSearch && <QuickSearch instances={snapshot.instances} pages={searchPages} onClose={() => setQuickSearch(false)}
      onNavigate={(next) => { setQuickSearch(false); navigate(next); }}
      onManage={(instance) => { setQuickSearch(false); openManager(instance); }} />}
    {sessionsOpen && <Modal title="Minecraft sessions" description="Open a session to see its status, logs, and stop controls." onClose={() => setSessionsOpen(false)}>
      <div className="quick-search-results">
        {activeGames.map((instance) => <button key={instance.id} className="quick-result" onClick={() => { setSessionsOpen(false); openManager(instance); }}>
          <Activity size={16} /><span><strong>{instance.name}</strong><small>Minecraft {instance.version} · {readinessLabel[instance.readiness]}</small></span><ArrowRight size={15} />
        </button>)}
        {activeGames.length === 0 && <p className="panel-footnote">All Minecraft sessions have ended.</p>}
      </div>
    </Modal>}
    {managed && !editor && !removing && <InstanceManager key={managed.id} instance={managed} status={game.status(managed)}
      active={managed.id === snapshot.activeInstanceId} offline={accounts?.offline ?? false} synced={snapshot.settings.syncRocketClient} busy={busy} notice={launcher.toast}
      stopping={game.stopping.has(managed.id)} showDiagnostics={managerDiagnostics} onStop={() => void game.abort(managed)}
      onClose={() => setManagedId(null)} onPlay={() => play(managed)} onInstall={() => void game.run(managed, 'install')}
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
