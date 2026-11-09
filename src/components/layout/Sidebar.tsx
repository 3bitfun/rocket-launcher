import { Boxes, Download, LayoutDashboard, Puzzle, Settings2, UserRound, Users, ArrowUpRight } from 'lucide-react';
import type { AccountsSummary, Page } from '../../types';
import { motion } from 'motion/react';

const navigation = [
  { id: 'launchpad', title: 'Launch Pad', icon: LayoutDashboard },
  { id: 'instances', title: 'Instances', icon: Boxes },
  { id: 'multiplayer', title: 'Multiplayer', icon: Users },
  { id: 'mods', title: 'Mods', icon: Puzzle },
  { id: 'downloads', title: 'Downloads', icon: Download },
  { id: 'settings', title: 'Settings', icon: Settings2 },
] as const;

export function Sidebar({ page, onNavigate, count, version, account, offline }: { page: Page; onNavigate: (page: Page) => void; count: number; version: string; account?: AccountsSummary['profiles'][number]; offline: boolean }) {
  const status = offline ? 'Offline mode' : account?.type === 'offline' ? 'Offline profile' : account?.type === 'local_test' ? 'Local test profile' : account ? 'Microsoft account' : 'Local workspace';
  return <aside className="sidebar">
    <div className="brand"><span className="brand-mark"><img src="/rocket.svg" alt="" /></span><div>Rocket<span>LAUNCHER</span></div></div>
    <nav aria-label="Main navigation">
      {['Play', 'Manage'].map((label, group) => <div className="nav-group" key={label}>
        <div className="nav-label">{label}</div>
        {navigation.slice(group * 3, group * 3 + 3).map(({ id, title, icon: Icon }) =>
          <button type="button" key={id} className={`nav-item ${page === id ? 'active' : ''}`} aria-current={page === id ? 'page' : undefined} onClick={() => onNavigate(id)}>
            {page === id && <motion.span className="nav-selection" layoutId="launcher-navigation-selection" aria-hidden="true" />}
            <Icon size={18} strokeWidth={1.8} /><span>{title}</span>
            {id === 'instances' && count > 0 && <span className="nav-count">{count}</span>}
          </button>)}
      </div>)}
    </nav>
    <div className="sidebar-bottom">
      <button className="account-button" onClick={() => onNavigate('settings')}><span className="avatar"><UserRound size={19} /></span><span><strong>{account?.name ?? 'Set up a player profile'}</strong><small><span className={`status-dot ${account?.type === 'microsoft' && !offline ? 'online' : ''}`} />{status}</small></span><ArrowUpRight size={15} /></button>
      <div className="sidebar-footer"><span>Rocket Launcher</span><span>v{version}</span></div>
    </div>
  </aside>;
}
