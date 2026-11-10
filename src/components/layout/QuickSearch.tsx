import { useRef, useState } from 'react';
import { ArrowRight, Boxes, Search, Star } from 'lucide-react';
import { Modal } from '../ui/Modal';
import { loaderName } from '../../services/loaders';
import { readinessLabel } from '../../services/readiness';
import type { Instance, Page } from '../../types';

export function QuickSearch({ instances, pages, onNavigate, onManage, onClose }: {
  instances: Instance[];
  pages: { id: Page; title: string }[];
  onNavigate: (page: Page) => void;
  onManage: (instance: Instance) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState('');
  const results = useRef<HTMLDivElement>(null);
  const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const matches = (label: string) => terms.every((term) => label.toLowerCase().includes(term));
  const found = instances
    .filter((instance) => matches(`${instance.name} ${instance.version} ${instance.loader}`))
    .sort((a, b) => Number(Boolean(b.favorite)) - Number(Boolean(a.favorite)) || b.lastSelectedAt - a.lastSelectedAt);
  const destinations = pages.filter((page) => matches(page.title));
  return <Modal title="Quick search" description="Find an instance or jump to a launcher page." onClose={onClose}>
    <label className="search-field quick-search-input"><Search size={18} /><input autoFocus aria-label="Search launcher" placeholder="Instance name, version, or page…" value={query}
      onChange={(event) => setQuery(event.target.value)} onKeyDown={(event) => {
        const first = results.current?.querySelector('button');
        if (!first) return;
        if (event.key === 'Enter') { event.preventDefault(); first.click(); }
        if (event.key === 'ArrowDown') { event.preventDefault(); first.focus(); }
      }} /></label>
    <div ref={results} className="quick-search-results" onKeyDown={(event) => {
      if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
      const buttons = [...event.currentTarget.querySelectorAll('button')];
      const index = buttons.findIndex((button) => button === document.activeElement);
      if (index < 0) return;
      event.preventDefault();
      buttons[index + (event.key === 'ArrowDown' ? 1 : -1)]?.focus();
    }}>
      {found.length > 0 && <section aria-label="Matching instances">
        <h3>Instances</h3>
        {found.map((instance) => <button className="quick-result" key={instance.id} onClick={() => onManage(instance)}>
          {instance.favorite ? <Star size={16} className="favorite-icon" /> : <Boxes size={16} />}
          <span><strong>{instance.name}</strong><small>Minecraft {instance.version} · {loaderName(instance.loader)} · {readinessLabel[instance.readiness]}</small></span>
          <ArrowRight size={15} />
        </button>)}
      </section>}
      {destinations.length > 0 && <section aria-label="Matching pages">
        <h3>Pages</h3>
        {destinations.map((page) => <button className="quick-result" key={page.id} onClick={() => onNavigate(page.id)}><span><strong>{page.title}</strong></span><ArrowRight size={15} /></button>)}
      </section>}
      {!found.length && !destinations.length && <p className="panel-footnote" role="status">No matches. Try another name or version.</p>}
    </div>
  </Modal>;
}
