import { useMemo, useState } from 'react';
import { Boxes, Plus, Search } from 'lucide-react';
import { InstanceCard } from '../../components/instances/InstanceCard';
import { EmptyState } from '../../components/ui/Primitives';
import type { AppSnapshot, Instance } from '../../types';

export function Instances({ snapshot, busy, onCreate, onEdit, onSelect, onDuplicate, onDelete }: {
  snapshot: AppSnapshot; busy: boolean; onCreate: () => void; onEdit: (instance: Instance) => void; onSelect: (instance: Instance) => void; onDuplicate: (instance: Instance) => void; onDelete: (instance: Instance) => void;
}) {
  const [search, setSearch] = useState('');
  const [sort, setSort] = useState('recent');
  const instances = useMemo(() => snapshot.instances.filter((instance) => `${instance.name} ${instance.version}`.toLowerCase().includes(search.toLowerCase())).sort((a, b) => sort === 'name' ? a.name.localeCompare(b.name) : b.lastSelectedAt - a.lastSelectedAt), [snapshot.instances, search, sort]);
  return <>
    <div className="page-intro"><div><h2>A world for every mood.</h2><p>Separate settings. Separate saves. Endless possibilities.</p></div><button className="button primary" onClick={onCreate} disabled={busy}><Plus size={17} />Create instance</button></div>
    <div className="toolbar"><label className="search-field"><Search size={17} /><input aria-label="Search instances" placeholder="Find an instance…" value={search} onChange={(e) => setSearch(e.target.value)} /></label><span className="toolbar-count">{instances.length} {instances.length === 1 ? 'instance' : 'instances'}</span><select aria-label="Sort instances" value={sort} onChange={(e) => setSort(e.target.value)}><option value="recent">Recently selected</option><option value="name">Name A–Z</option></select></div>
    {instances.length ? <div className="instance-grid">{instances.map((instance) => <InstanceCard key={instance.id} instance={instance} active={instance.id === snapshot.activeInstanceId} busy={busy} onSelect={() => onSelect(instance)} onEdit={() => onEdit(instance)} onDuplicate={() => onDuplicate(instance)} onDelete={() => onDelete(instance)} />)}</div>
      : <div className="panel"><EmptyState icon={<Boxes size={29} />} title={search ? 'No matching worlds' : 'Your universe starts here'} description={search ? 'Try another name or Minecraft version.' : 'Create your first instance with an official Minecraft version and a dedicated game directory.'}>{search ? <button className="button secondary" onClick={() => setSearch('')}>Clear search</button> : <button className="button primary" onClick={onCreate}><Plus size={16} />Create instance</button>}</EmptyState></div>}
    <div className="notice page-notice"><Boxes size={17} /><p>Duplicating copies the configuration into a new, empty game directory. Removing an instance preserves its files on disk.</p></div>
  </>;
}
