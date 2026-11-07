import { Boxes, Download, LayoutDashboard, Puzzle, Rocket, Settings2, UserRound, ArrowUpRight } from 'lucide-react';
import { motion } from 'motion/react';
import type { AccountsSummary, Page } from '../../types';

const navigation = [
  { id: 'launchpad', title: 'Launch Pad', icon: LayoutDashboard },
  { id: 'instances', title: 'Instances', icon: Boxes },
  { id: 'mods', title: 'Mods', icon: Puzzle },
  { id: 'downloads', title: 'Downloads', icon: Download },
  { id: 'settings', title: 'Settings', icon: Settings2 },
] as const;

export function Sidebar({ page, onNavigate, count, version, account, offline }: { page: Page; onNavigate: (page: Page) => void; count: number; version: string; account?: AccountsSummary['profiles'][number]; offline: boolean }) {
  const status = offline ? 'Offline mode' : account?.type === 'local_test' ? 'Local test profile' : account ? 'Microsoft account' : 'Local workspace';
  return <aside className="sidebar">
    <div className="brand"><span className="brand-mark"><Rocket size={23} strokeWidth={1.8} /></span><div>rocket<span>LAUNCHER</span></div></div>
    <div className="nav-label">YOUR WORKSPACE</div>
    <nav aria-label="Main navigation">{navigation.map(({ id, title, icon: Icon }) => <button key={id} className={`nav-item ${page === id ? 'active' : ''}`} aria-current={page === id ? 'page' : undefined} onClick={() => onNavigate(id)}>
      {page === id && <motion.span className="nav-indicator" layoutId="active-navigation" />}
      <Icon size={19} strokeWidth={1.7} /><span>{title}</span>{id === 'instances' && count > 0 && <span className="nav-count">{count}</span>}
    </button>)}</nav>
    <div className="sidebar-bottom"><div className="build-note"><span className="eyebrow">ROCKET LAUNCHER</span><p>A little space for<br />your next big world.</p></div>
      <button className="account-button" onClick={() => onNavigate('settings')}><span className="avatar"><UserRound size={19} /></span><span><strong>{account?.name ?? 'No account connected'}</strong><small><span className={`status-dot ${account?.type === 'microsoft' && !offline ? 'online' : ''}`} />{status}</small></span><ArrowUpRight size={15} /></button>
      <div className="sidebar-footer"><span>Rocket Launcher</span><span>v{version}</span></div>
    </div>
  </aside>;
}
