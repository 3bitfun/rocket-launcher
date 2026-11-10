import { useMemo, useState } from 'react';
import { Boxes, ChevronDown, Folder, Plus, Search, Star } from 'lucide-react';
import { InstanceCard } from '../../components/instances/InstanceCard';
import { EmptyState } from '../../components/ui/Primitives';
import type { AppSnapshot, Instance } from '../../types';

export function Instances({ snapshot, busy, onCreate, onEdit, onManage, onDuplicate, onDelete, onFavorite }: {
  snapshot: AppSnapshot; busy: boolean; onCreate: () => void; onEdit: (instance: Instance) => void; onManage: (instance: Instance) => void; onDuplicate: (instance: Instance) => void; onDelete: (instance: Instance) => void; onFavorite: (instance: Instance) => void;
}) {
  const [search, setSearch] = useState('');
  const [sort, setSort] = useState('recent');
  const [rocketExpanded, setRocketExpanded] = useState(true);
  const [favoritesOnly, setFavoritesOnly] = useState(false);
  const instances = useMemo(() => snapshot.instances
    .filter((instance) => (!favoritesOnly || instance.favorite) && `${instance.name} ${instance.version} ${instance.loader}`.toLowerCase().includes(search.trim().toLowerCase()))
    .sort((a, b) => Number(Boolean(b.favorite)) - Number(Boolean(a.favorite))
      || (sort === 'name' ? a.name.localeCompare(b.name) : b.lastSelectedAt - a.lastSelectedAt)), [snapshot.instances, search, sort, favoritesOnly]);
  const rocket = instances.filter((instance) => instance.rocketPack);
  const others = instances.filter((instance) => !instance.rocketPack);
  const pendingMoves = rocket.filter((instance) => instance.rocketFolderPending).length;
  const filtering = Boolean(search.trim()) || favoritesOnly;
  const expanded = rocketExpanded || filtering;
  function renderCard(instance: Instance) {
    return <InstanceCard key={instance.id} instance={instance} active={instance.id === snapshot.activeInstanceId} busy={busy}
      onManage={() => onManage(instance)} onEdit={() => onEdit(instance)} onDuplicate={() => onDuplicate(instance)} onDelete={() => onDelete(instance)} onFavorite={() => onFavorite(instance)} />;
  }
  return <>
    <div className="page-intro"><div><h2>Your instances</h2><p>Open an instance to play, manage its mods, or change its settings.</p></div><button className="button primary" onClick={onCreate} disabled={busy}><Plus size={17} />Create instance</button></div>
    <div className="toolbar"><label className="search-field"><Search size={17} /><input aria-label="Search instances" placeholder="Find by name, version, or loader…" value={search} onChange={(e) => setSearch(e.target.value)} /></label>
      <button className="button secondary small-button favorite-button" aria-pressed={favoritesOnly} onClick={() => setFavoritesOnly(!favoritesOnly)}><Star size={15} /> Favorites</button>
      <span className="toolbar-count">{instances.length} {instances.length === 1 ? 'instance' : 'instances'}</span><select aria-label="Sort instances" value={sort} onChange={(e) => setSort(e.target.value)}><option value="recent">Recently selected</option><option value="name">Name A–Z</option></select></div>
    {rocket.length > 0 && <section className="rocket-folder" aria-label="Rocket folder">
      <button className="button secondary rocket-folder-toggle" type="button" aria-expanded={expanded} aria-controls="rocket-folder-instances" onClick={() => setRocketExpanded(!expanded)} disabled={filtering}>
        <Folder size={18} /><strong>Rocket</strong><span className="tag">{rocket.length}</span><ChevronDown size={16} />
      </button>
      {pendingMoves > 0 && <p className="panel-footnote">{pendingMoves} Rocket Pack {pendingMoves === 1 ? 'instance is' : 'instances are'} still at the original location. Restart Rocket Launcher to retry the move. Folders on a different drive stay in place.</p>}
      <div id="rocket-folder-instances" hidden={!expanded}>{expanded && <div className="instance-grid">{rocket.map(renderCard)}</div>}</div>
    </section>}
    {instances.length ? others.length > 0 && <div className="instance-grid">{others.map(renderCard)}</div>
      : <div className="panel"><EmptyState icon={<Boxes size={29} />} title={filtering ? 'No matching instances' : 'No instances yet'} description={favoritesOnly ? 'Star an instance to add it to Favorites, or clear your filters.' : filtering ? 'Try another name, version, or loader.' : 'Each instance has its own game folder and saves.'}>{filtering && <button className="button secondary" onClick={() => { setSearch(''); setFavoritesOnly(false); }}>Clear filters</button>}</EmptyState></div>}
    <div className="notice page-notice"><Boxes size={17} /><p>Duplicating copies the configuration into a new, empty game directory. Removing an instance preserves its files on disk.</p></div>
  </>;
}
