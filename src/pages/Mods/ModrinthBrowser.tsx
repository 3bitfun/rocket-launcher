import { useEffect, useState } from 'react';
import { ArrowLeft, ArrowRight, Download, Puzzle, Search } from 'lucide-react';
import { Spinner } from '../../components/ui/Primitives';
import { errorMessage, native } from '../../services/native';
import type { Instance, ModrinthPage, ModrinthProject, ModrinthVersion } from '../../types';

const pageSize = 20;
const counts = new Intl.NumberFormat(undefined, { notation: 'compact' });

function ModIcon({ project }: { project: ModrinthProject }) {
  const [failed, setFailed] = useState(false);
  return <span className="modrinth-icon">
    {project.iconUrl?.startsWith('https://cdn.modrinth.com/') && !failed
      ? <img src={project.iconUrl} alt="" loading="lazy" onError={() => setFailed(true)} />
      : <Puzzle size={22} />}
  </span>;
}

export function ModrinthBrowser({ instance, onInstalled }: { instance: Instance; onInstalled: () => void }) {
  const [draft, setDraft] = useState('');
  const [search, setSearch] = useState({ query: '', offset: 0, sort: '' });
  const [page, setPage] = useState<ModrinthPage | null>(null);
  const [project, setProject] = useState<ModrinthProject | null>(null);
  const [versions, setVersions] = useState<ModrinthVersion[] | null>(null);
  const [selectedId, setSelectedId] = useState('');
  const [searchError, setSearchError] = useState('');
  const [versionError, setVersionError] = useState('');
  const [installing, setInstalling] = useState(false);
  const [installed, setInstalled] = useState('');
  const version = versions?.find((item) => item.id === selectedId);
  const instanceId = instance.id;

  useEffect(() => {
    let active = true;
    setPage(null);
    setSearchError('');
    void native.searchMods(instanceId, search.query, search.offset, search.sort || undefined)
      .then((result) => { if (active) setPage(result); })
      .catch((error) => { if (active) setSearchError(errorMessage(error)); });
    return () => { active = false; };
  }, [instanceId, search]);

  useEffect(() => {
    if (!project) return;
    let active = true;
    setVersions(null);
    setVersionError('');
    setSelectedId('');
    setInstalled('');
    void native.modVersions(instanceId, project.projectId)
      .then((items) => {
        if (!active) return;
        setVersions(items);
        setSelectedId((items.find((item) => item.versionType === 'release') ?? items[0])?.id ?? '');
      })
      .catch((error) => { if (active) setVersionError(errorMessage(error)); });
    return () => { active = false; };
  }, [instanceId, project]);

  async function install() {
    if (!project || !version || installing) return;
    setInstalling(true);
    setVersionError('');
    setInstalled('');
    try {
      const filename = await native.installMod(instanceId, project.projectId, version.id);
      setInstalled(`Installed ${filename}`);
      onInstalled();
    } catch (error) {
      setVersionError(errorMessage(error));
    } finally {
      setInstalling(false);
    }
  }

  return <section className="panel mods-panel">
    <div className="section-heading"><h2>Browse Modrinth</h2><span className="tag">Minecraft {instance.version} · Fabric</span></div>
    {project ? <>
      <button type="button" className="text-button" disabled={installing} onClick={() => setProject(null)}><ArrowLeft size={15} /> Back to results</button>
      <div className="modrinth-heading"><ModIcon key={project.projectId} project={project} /><div><h3>{project.title}</h3><small>by {project.author} · {counts.format(project.downloads)} downloads</small></div></div>
      <p>{project.description}</p>
      {project.featuredGallery?.startsWith('https://cdn.modrinth.com/') && <img className="modrinth-gallery" src={project.featuredGallery} alt={`Featured screenshot for ${project.title}`} loading="lazy" />}
      {!versions && !versionError && <div className="loading-inline"><Spinner />Finding matching versions…</div>}
      {versions?.length === 0 && <p>No downloadable Fabric JAR matches Minecraft {instance.version} for this project.</p>}
      {versions && versions.length > 0 && <div className="modrinth-version">
        <label className="field"><span>Compatible version</span><select value={selectedId} disabled={installing} onChange={(event) => {
          setSelectedId(event.target.value); setInstalled(''); setVersionError('');
        }}>{versions.map((item) => <option key={item.id} value={item.id}>{item.versionNumber} · {item.versionType}</option>)}</select></label>
        {version && <p className="panel-footnote">{version.fileName}</p>}
        {!!version?.requiredDependencies.length && <p className="panel-footnote">Includes {version.requiredDependencies.length} required {version.requiredDependencies.length === 1 ? 'dependency' : 'dependencies'}.</p>}
        <button type="button" className="button primary small-button" onClick={() => void install()} disabled={!version || installing || !!installed}>
          {installing ? <Spinner size={15} /> : <Download size={15} />} {installing ? 'Downloading and verifying…' : 'Install mod'}
        </button>
      </div>}
      {installed && <p role="status">{installed}</p>}
      {versionError && <p className="form-error" role="alert">{versionError}</p>}
    </> : <>
      <form className="modrinth-search" onSubmit={(event) => {
        event.preventDefault();
        setSearch({ ...search, query: draft.trim(), offset: 0 });
      }}>
        <label className="field"><span>Search mods</span><input value={draft} maxLength={100} onChange={(event) => setDraft(event.target.value)} placeholder="Sodium, Iris, Fabric API…" /></label>
        <label className="field"><span>Sort by</span><select value={search.sort} onChange={(event) => setSearch({ ...search, sort: event.target.value, offset: 0 })}><option value="">Best match</option><option value="downloads">Most downloaded</option><option value="updated">Recently updated</option></select></label>
        <button type="submit" className="button secondary"><Search size={16} /> Search</button>
      </form>
      {!page && !searchError && <div className="loading-inline"><Spinner />Searching Modrinth…</div>}
      {searchError && <p className="form-error" role="alert">{searchError}</p>}
      {page?.hits.length === 0 && <p>No mods found. Try another search.</p>}
      {!!page?.hits.length && <ul className="mod-list modrinth-results">{page.hits.map((hit) => <li key={hit.projectId}>
        <ModIcon project={hit} />
        <div><strong>{hit.title}</strong> <small>by {hit.author} · {counts.format(hit.downloads)} downloads</small><p>{hit.description}</p>
          {!!hit.displayCategories?.length && <div className="modrinth-tags">{hit.displayCategories.slice(0, 4).map((category) => <span className="tag" key={category}>{category}</span>)}</div>}
        </div>
        <button type="button" className="button secondary small-button" onClick={() => setProject(hit)} aria-label={`View versions of ${hit.title}`}>Versions <ArrowRight size={14} /></button>
      </li>)}</ul>}
      {page && (search.offset > 0 || page.totalHits > pageSize) && <div className="inline-actions">
        <button className="button secondary small-button" type="button" disabled={search.offset === 0} onClick={() => setSearch({ ...search, offset: search.offset - pageSize })}>Previous</button>
        <span>Page {search.offset / pageSize + 1}</span>
        <button className="button secondary small-button" type="button" disabled={search.offset + pageSize >= Math.min(page.totalHits, 1000)} onClick={() => setSearch({ ...search, offset: search.offset + pageSize })}>Next</button>
      </div>}
    </>}
  </section>;
}
