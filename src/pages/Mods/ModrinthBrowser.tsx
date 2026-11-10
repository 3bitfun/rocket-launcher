import { useEffect, useState } from 'react';
import { ArrowLeft, ArrowRight, Download, Puzzle, Search } from 'lucide-react';
import { Spinner } from '../../components/ui/Primitives';
import { errorMessage, native } from '../../services/native';
import { loaderName } from '../../services/loaders';
import type { Instance, ModpackDetails, ModpackFilters, ModpackProgress, ModrinthPage, ModrinthProject, ModrinthVersion } from '../../types';

const pageSize = 20;
const counts = new Intl.NumberFormat(undefined, { notation: 'compact' });
const loaders = ['fabric', 'forge', 'neoforge', 'quilt'];
const categories = ['adventure', 'challenging', 'combat', 'kitchen-sink', 'lightweight', 'magic', 'multiplayer', 'optimization', 'quests', 'technology'];
const emptyFilters: ModpackFilters = { game: '', loader: '', category: '', side: '', collection: '' };

const imageHosts = new Set(['cdn.modrinth.com', 'cdn-raw.modrinth.com', 'media.forgecdn.net', 'mediafilez.forgecdn.net']);

function imageSources(urls: (string | null | undefined)[]) {
  return [...new Set(urls.filter((value): value is string => {
    if (!value) return false;
    try {
      const url = new URL(value);
      return url.protocol === 'https:' && !url.username && !url.password && !url.port && imageHosts.has(url.hostname);
    } catch { return false; }
  }))];
}

function ProjectImage({ urls, alt, className }: { urls: (string | null | undefined)[]; alt: string; className: string }) {
  const sources = imageSources(urls);
  return <ImagePreview key={JSON.stringify(sources)} sources={sources} alt={alt} className={className} />;
}

function ImagePreview({ sources, alt, className }: { sources: string[]; alt: string; className: string }) {
  const [index, setIndex] = useState(0);
  const [ready, setReady] = useState(false);
  return <div className={`${className} project-image ${sources[index] && !ready ? 'image-loading' : ''}`}>
    {sources[index] ? <img key={sources[index]} src={sources[index]} alt={alt} loading="lazy" decoding="async"
      onLoad={() => setReady(true)} onError={() => { setReady(false); setIndex(index + 1); }}
      className={ready ? 'image-ready' : ''} /> : <Puzzle size={26} aria-hidden="true" />}
  </div>;
}

function ModIcon({ project }: { project: ModrinthProject }) {
  return <ProjectImage className="modrinth-icon" urls={[project.iconUrl, project.featuredGallery, ...(project.gallery ?? [])]} alt="" />;
}

export function ModrinthBrowser({ instance, gameVersions = [], onInstalled, onPackInstalled }: {
  instance?: Instance; gameVersions?: string[]; onInstalled: () => void; onPackInstalled?: () => void;
}) {
  const [kind, setKind] = useState<'mod' | 'modpack'>(instance ? 'mod' : 'modpack');
  const [source, setSource] = useState<'modrinth' | 'curseforge'>('modrinth');
  const curseForge = source === 'curseforge';
  const provider = curseForge ? 'CurseForge' : 'Modrinth';
  const pack = !curseForge && (kind === 'modpack' || !instance);
  const [previews, setPreviews] = useState<Record<string, string[]>>({});
  const [apiKey, setApiKey] = useState('');
  const [keySaved, setKeySaved] = useState(false);
  const [keyBusy, setKeyBusy] = useState(false);
  const [keyError, setKeyError] = useState('');
  const [filters, setFilters] = useState(emptyFilters);
  const [details, setDetails] = useState<ModpackDetails | null>(null);
  const [game, setGame] = useState('');
  const [packLoader, setPackLoader] = useState('');
  const [progress, setProgress] = useState<ModpackProgress | null>(null);
  const [retry, setRetry] = useState(0);
  const [detailRetry, setDetailRetry] = useState(0);
  const [detailsFailed, setDetailsFailed] = useState(false);
  const [draft, setDraft] = useState('');
  const [search, setSearch] = useState({ query: '', offset: 0, sort: '' });
  const [page, setPage] = useState<ModrinthPage | null>(null);
  const [project, setProject] = useState<ModrinthProject | null>(null);
  const [versions, setVersions] = useState<ModrinthVersion[] | null>(null);
  const [selectedId, setSelectedId] = useState('');
  const [searchError, setSearchError] = useState('');
  const [versionError, setVersionError] = useState('');
  const [installing, setInstalling] = useState<string | null>(null);
  const [installed, setInstalled] = useState('');
  const version = versions?.find((item) => item.id === selectedId);
  const instanceId = pack ? undefined : instance?.id;
  const instanceVersion = pack ? undefined : instance?.version;
  const instanceLoader = pack ? undefined : instance?.loader;
  const instanceLoaderVersion = pack ? undefined : instance?.loaderVersion;
  const packVersion = details?.versions.find((item) => item.id === selectedId);

  useEffect(() => {
    if (!curseForge) return;
    let active = true;
    void native.curseForgeKeySaved().then((saved) => {
      if (active) setKeySaved(saved);
    }).catch((error) => { if (active) setKeyError(errorMessage(error)); });
    return () => { active = false; };
  }, [curseForge]);

  async function changeKey(key: string | null) {
    setKeyBusy(true);
    setKeyError('');
    try {
      await native.setCurseForgeKey(key);
      setApiKey('');
      setKeySaved(key !== null);
      setRetry((value) => value + 1);
    } catch (error) {
      setKeyError(errorMessage(error));
    } finally {
      setKeyBusy(false);
    }
  }

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setSearch((value) => value.query === draft.trim() ? value : { ...value, query: draft.trim(), offset: 0 });
    }, 320);
    return () => window.clearTimeout(timer);
  }, [draft]);

  useEffect(() => {
    let active = true;
    setPage(null);
    setSearchError('');
    if (!pack && !instanceId) return;
    const request = pack
      ? native.searchModpacks(search.query, search.offset, search.sort || 'relevance', filters)
      : (curseForge ? native.searchCurseForge : native.searchMods)(instanceId!, search.query, search.offset, search.sort || undefined);
    void request
      .then((result) => { if (active) setPage(result); })
      .catch((error) => { if (active) setSearchError(errorMessage(error)); });
    return () => { active = false; };
  }, [instanceId, instanceVersion, instanceLoader, instanceLoaderVersion, pack, curseForge, filters, search, retry]);

  useEffect(() => {
    if (!project) return;
    let active = true;
    setVersions(null);
    setDetails(null);
    setDetailsFailed(false);
    setVersionError('');
    setSelectedId('');
    setInstalled('');
    if (pack) {
      void native.modpackDetails(project.projectId).then((result) => {
        if (!active) return;
        setDetails(result);
        const selected = result.versions.find((item) => item.versionType === 'release' && item.loaders.some((loader) => loader === 'fabric' || loader === 'forge')) ?? result.versions[0];
        setSelectedId(selected?.id ?? '');
        setGame(selected?.gameVersions.includes(filters.game) ? filters.game : selected?.gameVersions[0] ?? '');
        setPackLoader(selected?.loaders.includes(filters.loader) ? filters.loader : selected?.loaders[0] ?? '');
      }).catch((error) => { if (active) { setDetailsFailed(true); setVersionError(errorMessage(error)); } });
      return () => { active = false; };
    }
    if (!instanceId) return;
    void (curseForge ? native.curseForgeVersions : native.modVersions)(instanceId, project.projectId)
      .then((items) => {
        if (!active) return;
        setVersions(items);
        setSelectedId((items.find((item) => item.versionType === 'release') ?? items[0])?.id ?? '');
      })
      .catch((error) => { if (active) { setDetailsFailed(true); setVersionError(errorMessage(error)); } });
    return () => { active = false; };
  }, [instanceId, instanceVersion, instanceLoader, instanceLoaderVersion, project, pack, curseForge, filters, detailRetry]);

  useEffect(() => {
    if (!project || pack || curseForge || previews[project.projectId]) return;
    let active = true;
    void native.modrinthGallery(project.projectId).then((images) => {
      if (active) setPreviews((current) => ({ ...current, [project.projectId]: images }));
    }).catch(() => {});
    return () => { active = false; };
  }, [project, pack, curseForge, previews]);

  async function install(target: ModrinthProject, versionId?: string) {
    if (installing || !instanceId) return;
    setInstalling(target.projectId);
    setVersionError('');
    setInstalled('');
    try {
      const filename = await (curseForge ? native.installCurseForge : native.installMod)(instanceId, target.projectId, versionId);
      setInstalled(`Installed ${filename}`);
      onInstalled();
    } catch (error) {
      setVersionError(errorMessage(error));
    } finally {
      setInstalling(null);
    }
  }

  async function installPack() {
    if (installing || !project || !packVersion) return;
    setInstalling(project.projectId);
    setVersionError('');
    setProgress({ phase: 'Preparing the modpack', completed: 0, total: 0 });
    try {
      await native.installModpack(project.projectId, packVersion.id, game, packLoader, setProgress);
      setInstalled(`Created an instance for ${project.title} ${packVersion.versionNumber}.`);
      onPackInstalled?.();
    } catch (error) {
      setVersionError(errorMessage(error));
    } finally {
      setInstalling(null);
      setProgress(null);
    }
  }

  function filterBy(key: keyof ModpackFilters, value: string) {
    setFilters((current) => ({ ...current, [key]: value }));
    setSearch((current) => ({ ...current, offset: 0 }));
  }

  const filterOptions: { key: keyof ModpackFilters; label: string; options: string[] }[] = [
    { key: 'game', label: 'Minecraft version', options: gameVersions },
    { key: 'loader', label: 'Loader', options: loaders },
    { key: 'category', label: 'Category', options: categories },
    { key: 'side', label: 'Compatibility', options: ['client', 'server'] },
    { key: 'collection', label: 'Collection', options: ['featured', 'popular'] },
  ];

  return <section className="panel mods-panel">
    <div className="section-heading"><h2>Browse {provider}</h2>{!pack && instance && <span className="tag">Minecraft {instance.version} · {loaderName(instance.loader)}</span>}</div>
    <div className="segmented modrinth-kind" role="group" aria-label="Mod source">
      {(['modrinth', 'curseforge'] as const).map((item) => <button type="button" key={item} className={source === item ? 'active' : ''} aria-pressed={source === item} disabled={!!installing || keyBusy}
        onClick={() => {
          if (item === source) return;
          setSource(item); setProject(null); setPage(null); setSearchError(''); setInstalled(''); setVersionError('');
          setSearch({ query: draft.trim(), offset: 0, sort: '' });
        }}>{item === 'modrinth' ? 'Modrinth' : 'CurseForge'}</button>)}
    </div>
    {curseForge && !project && <details className="pack-description">
      <summary>API key setup{keySaved ? ' · Key saved' : ''}</summary>
      <p>Use your CurseForge for Studios API key. Rocket Launcher stores it in Windows Credential Manager.</p>
      <form className="modrinth-search" onSubmit={(event) => {
        event.preventDefault();
        if (apiKey.trim()) void changeKey(apiKey.trim());
      }}>
        <label className="field"><span>CurseForge API key</span><input type="password" value={apiKey} maxLength={512} autoComplete="off"
          onChange={(event) => setApiKey(event.target.value)} /></label>
        <button type="submit" className="button secondary small-button" disabled={keyBusy || !apiKey.trim()}>Save key</button>
        {keySaved && <button type="button" className="button secondary small-button" disabled={keyBusy} onClick={() => void changeKey(null)}>Remove saved key</button>}
      </form>
      {keyError && <p className="form-error" role="alert">{keyError}</p>}
    </details>}
    {!project && !curseForge && <div className="segmented modrinth-kind" role="group" aria-label="Project type">
      <button type="button" className={!pack ? 'active' : ''} aria-pressed={!pack} disabled={!instance || !!installing} onClick={() => { setKind('mod'); setSearch({ query: draft.trim(), offset: 0, sort: '' }); }}>Mods</button>
      <button type="button" className={pack ? 'active' : ''} aria-pressed={pack} disabled={!!installing} onClick={() => { setKind('modpack'); setSearch({ query: draft.trim(), offset: 0, sort: '' }); }}>Modpacks</button>
    </div>}
    {curseForge && !instance ? <p className="panel-footnote">Choose a Fabric or Forge target instance to browse compatible CurseForge mods.</p> : project ? <>
      <button type="button" className="text-button" disabled={!!installing} onClick={() => setProject(null)}><ArrowLeft size={15} /> Back to results</button>
      <div className="modrinth-heading"><ModIcon key={project.projectId} project={project} /><div><h3>{project.title}</h3><small>by {project.author} · {counts.format(project.downloads)} downloads</small></div></div>
      <p>{project.description}</p>
      {pack && details && <>
        <div className="pack-gallery">{[...details.gallery].sort((a, b) => Number(b.featured) - Number(a.featured)).map((image) =>
          <figure key={image.url}><ProjectImage urls={[image.url]} alt={image.title ?? `Screenshot of ${project.title}`} className="pack-screenshot" />
            {(image.title || image.description) && <figcaption>{image.title ?? image.description}</figcaption>}
          </figure>)}</div>
        {details.body && <details className="pack-description" open><summary>About this modpack</summary><p>{details.body.replace(/!\[[^\]]*\]\([^)]*\)/g, '').replace(/\[([^\]]+)\]\([^)]*\)/g, '$1').trim()}</p></details>}
        {details.versions.length === 0 && <p className="panel-footnote">No downloadable .mrpack versions were found.</p>}
        {packVersion && <div className="modrinth-version">
          <div className="pack-filters">
            <label className="field"><span>Modpack release</span><select value={selectedId} disabled={!!installing} onChange={(e) => {
              const next = details.versions.find((item) => item.id === e.target.value);
              if (!next) return;
              setSelectedId(next.id); setGame(next.gameVersions[0] ?? ''); setPackLoader(next.loaders[0] ?? ''); setInstalled(''); setVersionError('');
            }}>{details.versions.map((item) => <option key={item.id} value={item.id}>{item.versionNumber} · {item.versionType} · {item.datePublished.slice(0, 10)}</option>)}</select></label>
            <label className="field"><span>Minecraft</span><select value={game} disabled={!!installing} onChange={(e) => { setGame(e.target.value); setInstalled(''); }}>{packVersion.gameVersions.map((item) => <option key={item}>{item}</option>)}</select></label>
            <label className="field"><span>Loader</span><select value={packLoader} disabled={!!installing} onChange={(e) => { setPackLoader(e.target.value); setInstalled(''); }}>{packVersion.loaders.map((item) => <option key={item}>{item}</option>)}</select></label>
          </div>
          {!!packVersion.requiredDependencies.length && <details className="pack-description"><summary>Required dependencies ({packVersion.requiredDependencies.length})</summary><ul>{packVersion.requiredDependencies.map((dependency, index) => <li key={index}>{dependency}</li>)}</ul></details>}
          <p className="panel-footnote">{packLoader === 'fabric' || packLoader === 'forge'
            ? 'Installs required client files into a new instance. To try an update, choose a newer release and keep your existing instance and worlds.'
            : 'This loader is available for browsing. Installation requires Fabric or a supported Forge release.'}</p>
          <button type="button" className="button primary" disabled={!game || !['fabric', 'forge'].includes(packLoader) || !!installing || !!installed} onClick={() => void installPack()}><Download size={16} /> Install in new instance</button>
          {progress && <div className="pack-progress" role="status"><p>{progress.phase}{progress.total ? ` · ${progress.completed} / ${progress.total}` : ''}</p><progress className="game-progress" max={progress.total || undefined} value={progress.total ? progress.completed : undefined} /></div>}
        </div>}
      </>}
      {!pack && <ProjectImage urls={[...(previews[project.projectId] ?? []), project.featuredGallery, ...(project.gallery ?? []), project.iconUrl]}
        alt={`Project preview for ${project.title}`} className="pack-screenshot" />}
      {(pack ? !details : !versions) && !detailsFailed && <div className="loading-inline"><Spinner />Loading project details…</div>}
      {detailsFailed && <button type="button" className="button secondary small-button" onClick={() => setDetailRetry((value) => value + 1)}>Retry details</button>}
      {!pack && versions?.length === 0 && instance && <p>No downloadable {loaderName(instance.loader)} JAR matches Minecraft {instance.version} for this project.</p>}
      {!pack && versions && versions.length > 0 && <div className="modrinth-version">
        <label className="field"><span>Compatible version</span><select value={selectedId} disabled={!!installing} onChange={(event) => {
          setSelectedId(event.target.value); setInstalled(''); setVersionError('');
        }}>{versions.map((item) => <option key={item.id} value={item.id}>{item.versionNumber} · {item.versionType}</option>)}</select></label>
        {version && <p className="panel-footnote">{version.fileName}</p>}
        {!!version?.requiredDependencies.length && <p className="panel-footnote">Includes {version.requiredDependencies.length} required {version.requiredDependencies.length === 1 ? 'dependency' : 'dependencies'}.</p>}
        <button type="button" className="button primary small-button" onClick={() => { if (version) void install(project, version.id); }} disabled={!version || !!installing || !!installed}>
          {installing ? <Spinner size={15} /> : <Download size={15} />} {installing ? 'Downloading and verifying…' : 'Install mod'}
        </button>
      </div>}
    </> : <>
      <form className="modrinth-search" onSubmit={(event) => {
        event.preventDefault();
        setSearch({ ...search, query: draft.trim(), offset: 0 });
      }}>
        <label className="field"><span>Search {pack ? 'modpacks' : 'mods'}</span><input value={draft} maxLength={100} onChange={(event) => setDraft(event.target.value)} placeholder={pack ? 'Search Modrinth modpacks…' : 'Sodium, Iris, Fabric API…'} /></label>
        <label className="field"><span>Sort by</span><select value={search.sort} onChange={(event) => setSearch({ ...search, sort: event.target.value, offset: 0 })}><option value="">{curseForge ? 'CurseForge order' : 'Best match'}</option><option value="downloads">Most downloaded</option><option value="updated">Recently updated</option>{pack && <><option value="follows">Most followed</option><option value="newest">Newest</option></>}</select></label>
        <button type="submit" className="button secondary"><Search size={16} /> Search</button>
      </form>
      {pack && <>
        <div className="pack-filters">{filterOptions.map(({ key, label, options }) => <label className="field" key={key}><span>{label}</span><select value={filters[key]} onChange={(e) => filterBy(key, e.target.value)}>
          <option value="">Any</option>{options.map((value) => <option key={value} value={value}>{value === 'featured' ? 'Rocket featured picks' : value === 'popular' ? 'Popular · 10,000+ downloads' : value}</option>)}
        </select></label>)}</div>
        {Object.values(filters).some(Boolean) && <div className="modrinth-tags" aria-label="Active filters">{filterOptions.filter(({ key }) => filters[key]).map(({ key, label }) =>
          <button className="tag" type="button" key={key} onClick={() => filterBy(key, '')} aria-label={`Remove ${label} filter`}>{filters[key]} ×</button>
        )}<button type="button" className="text-button" onClick={() => { setFilters(emptyFilters); setSearch({ ...search, offset: 0 }); }}>Clear all</button></div>}
      </>}
      {!page && !searchError && (pack ? <div className="pack-card-grid" role="status" aria-label="Searching Modrinth">{[0, 1, 2, 3].map((item) => <div key={item} className="pack-card pack-skeleton image-loading" />)}</div> : <div className="loading-inline"><Spinner />Searching {provider}…</div>)}
      {searchError && <div><p className="form-error" role="alert">{searchError}</p><button type="button" className="button secondary small-button" onClick={() => setRetry((value) => value + 1)}>Retry search</button></div>}
      {page?.hits.length === 0 && <p className="panel-footnote">No {pack ? 'modpacks' : 'mods'} found. Try another search or clear filters.</p>}
      {pack && !!page?.hits.length && <div className="pack-card-grid">{page.hits.map((hit) => <button type="button" className="pack-card" key={hit.projectId} onClick={() => setProject(hit)}>
        <ProjectImage urls={[hit.featuredGallery, ...(hit.gallery ?? []), hit.iconUrl]} alt="" className="pack-card-image" />
        <div className="pack-card-body"><div className="modrinth-heading"><ModIcon project={hit} /><strong>{hit.title}</strong></div>
          <p>{hit.description}</p><small>{counts.format(hit.downloads)} downloads{hit.dateModified && ` · Updated ${hit.dateModified.slice(0, 10)}`}</small>
          <div className="modrinth-tags">{(hit.categories ?? hit.displayCategories ?? []).filter((value) => loaders.includes(value)).map((value) => <span className="tag" key={value}>{value}</span>)}
            {(hit.versions ?? []).slice(-3).map((value) => <span className="tag" key={value}>{value}</span>)}
            {(hit.versions?.length ?? 0) > 3 && <span className="tag">+{hit.versions!.length - 3} versions</span>}
          </div>
        </div>
      </button>)}</div>}
      {!pack && !!page?.hits.length && <ul className="mod-list modrinth-results">{page.hits.map((hit) => <li key={hit.projectId}>
        <ModIcon project={hit} />
        <div><strong>{hit.title}</strong> <small>by {hit.author} · {counts.format(hit.downloads)} downloads</small><p>{hit.description}</p>
          {!!hit.displayCategories?.length && <div className="modrinth-tags">{hit.displayCategories.slice(0, 4).map((category) => <span className="tag" key={category}>{category}</span>)}</div>}
        </div>
        <div className="inline-actions modrinth-actions">
          <button type="button" className="button primary small-button" disabled={!!installing} onClick={() => void install(hit)} aria-label={`Install ${hit.title}`}>{installing === hit.projectId ? <Spinner size={14} /> : <Download size={14} />} Install</button>
          <button type="button" className="button secondary small-button" disabled={!!installing} onClick={() => setProject(hit)} aria-label={`View versions of ${hit.title}`}>Versions <ArrowRight size={14} /></button>
        </div>
      </li>)}</ul>}
      {page && (search.offset > 0 || page.totalHits > pageSize) && <div className="inline-actions">
        <button className="button secondary small-button" type="button" disabled={search.offset === 0} onClick={() => setSearch({ ...search, offset: search.offset - pageSize })}>Previous</button>
        <span>Page {search.offset / pageSize + 1}</span>
        <button className="button secondary small-button" type="button" disabled={search.offset + pageSize >= Math.min(page.totalHits, 1000)} onClick={() => setSearch({ ...search, offset: search.offset + pageSize })}>Next</button>
      </div>}
    </>}
    {installed && <p role="status">{installed}</p>}
    {versionError && <p className="form-error" role="alert">{versionError}</p>}
  </section>;
}
