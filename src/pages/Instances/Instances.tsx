import { useMemo, useState } from 'react';
import { Boxes, ChevronDown, Folder, Plus, Search } from 'lucide-react';
import { InstanceCard } from '../../components/instances/InstanceCard';
import { EmptyState } from '../../components/ui/Primitives';
import type { AppSnapshot, Instance } from '../../types';

export function Instances({ snapshot, busy, onCreate, onEdit, onManage, onDuplicate, onDelete }: {
  snapshot: AppSnapshot; busy: boolean; onCreate: () => void; onEdit: (instance: Instance) => void; onManage: (instance: Instance) => void; onDuplicate: (instance: Instance) => void; onDelete: (instance: Instance) => void;
}) {
  const [search, setSearch] = useState('');
  const [sort, setSort] = useState('recent');
  const [rocketExpanded, setRocketExpanded] = useState(true);
  const instances = useMemo(() => snapshot.instances.filter((instance) => `${instance.name} ${instance.version}`.toLowerCase().includes(search.toLowerCase())).sort((a, b) => sort === 'name' ? a.name.localeCompare(b.name) : b.lastSelectedAt - a.lastSelectedAt), [snapshot.instances, search, sort]);
  const rocket = instances.filter((instance) => instance.rocketPack);
  const others = instances.filter((instance) => !instance.rocketPack);
  const expanded = rocketExpanded || Boolean(search.trim());
  function renderCard(instance: Instance) {
    return <InstanceCard key={instance.id} instance={instance} active={instance.id === snapshot.activeInstanceId} busy={busy}
      onManage={() => onManage(instance)} onEdit={() => onEdit(instance)} onDuplicate={() => onDuplicate(instance)} onDelete={() => onDelete(instance)} />;
  }
  return <>
    <div className="page-intro"><div><h2>Your instances</h2><p>Open an instance to play, manage its mods, or change its settings.</p></div><button className="button primary" onClick={onCreate} disabled={busy}><Plus size={17} />Create instance</button></div>
    <div className="toolbar"><label className="search-field"><Search size={17} /><input aria-label="Search instances" placeholder="Find an instance…" value={search} onChange={(e) => setSearch(e.target.value)} /></label><span className="toolbar-count">{instances.length} {instances.length === 1 ? 'instance' : 'instances'}</span><select aria-label="Sort instances" value={sort} onChange={(e) => setSort(e.target.value)}><option value="recent">Recently selected</option><option value="name">Name A–Z</option></select></div>
    {rocket.length > 0 && <section className="rocket-folder" aria-label="Rocket folder">
      <button className="button secondary rocket-folder-toggle" type="button" aria-expanded={expanded} aria-controls="rocket-folder-instances" onClick={() => setRocketExpanded(!expanded)} disabled={Boolean(search.trim())}>
        <Folder size={18} /><strong>Rocket</strong><span className="tag">{rocket.length}</span><ChevronDown size={16} />
      </button>
      <div id="rocket-folder-instances" hidden={!expanded}>{expanded && <div className="instance-grid">{rocket.map(renderCard)}</div>}</div>
    </section>}
    {instances.length ? others.length > 0 && <div className="instance-grid">{others.map(renderCard)}</div>
      : <div className="panel"><EmptyState icon={<Boxes size={29} />} title={search ? 'No matching instances' : 'No instances yet'} description={search ? 'Try another name or Minecraft version.' : 'Each instance has its own game folder and saves.'}>{search && <button className="button secondary" onClick={() => setSearch('')}>Clear search</button>}</EmptyState></div>}
    <div className="notice page-notice"><Boxes size={17} /><p>Duplicating copies the configuration into a new, empty game directory. Removing an instance preserves its files on disk.</p></div>
  </>;
}
