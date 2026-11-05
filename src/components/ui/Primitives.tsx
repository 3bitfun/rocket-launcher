import type { ReactNode } from 'react';
import { LoaderCircle, FolderOpen, Terminal } from 'lucide-react';
import { errorMessage, native } from '../../services/native';

export function Spinner({ size = 18 }: { size?: number }) { return <LoaderCircle size={size} className="spinner" aria-hidden="true" />; }

export function EmptyState({ icon, title, description, children }: { icon: ReactNode; title: string; description: string; children?: ReactNode }) {
  return <div className="empty-state"><div className="empty-icon">{icon}</div><h3>{title}</h3><p>{description}</p>{children}</div>;
}

export function PathField({ label, value, onChange, kind = 'directory', hint, onError }: {
  label: string; value: string; onChange: (value: string) => void; kind?: 'directory' | 'java'; hint?: string; onError: (error: string) => void;
}) {
  async function choose() {
    try {
      const selected = await (kind === 'directory' ? native.chooseDirectory() : native.chooseJava());
      if (typeof selected === 'string') onChange(selected);
    } catch (error) { onError(errorMessage(error)); }
  }
  return <label className="field"><span>{label}</span><div className="path-field"><input value={value} onChange={(e) => onChange(e.target.value)} placeholder={kind === 'java' ? 'Automatic detection' : 'Choose a folder'} />
    <button type="button" className="icon-button" onClick={() => void choose()} aria-label={`Browse ${label.toLowerCase()}`}>{kind === 'java' ? <Terminal size={17} /> : <FolderOpen size={17} />}</button></div>{hint && <small>{hint}</small>}</label>;
}

export function Toggle({ checked, onChange, label, description, disabled = false }: { checked: boolean; onChange: (value: boolean) => void; label: string; description: string; disabled?: boolean }) {
  return <label className="toggle-row"><span><strong>{label}</strong><small>{description}</small></span><input className="switch" type="checkbox" role="switch" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} /></label>;
}

export function formatDate(time: number): string { return new Date(time * 1000).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' }); }
export function formatRam(mb: number): string { return `${mb / 1024} GB`; }
