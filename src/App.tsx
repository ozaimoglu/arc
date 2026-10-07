import { useCallback, useEffect, useRef, useState } from 'react';
import { flushSync } from 'react-dom';
import { ArrowDownWideNarrow, ArrowLeft, ArrowRight, Check, ChevronDown, ChevronLeft, ChevronRight, Clock3, EyeOff, FolderOpen, Gamepad2, Grid2X2, Heart, Image, List, LoaderCircle, MoreHorizontal, Pencil, Play, Plus, RefreshCw, Search, Settings2, X } from 'lucide-react';
import { api, imageSrc, isDesktop } from './bridge';
import { featuredGames, filterGames, lastPlayedLabel, playtimeLabel } from './library';
import Settings from './components/Settings';
import ArtworkPicker from './components/ArtworkPicker';
import EditGame from './components/EditGame';
import ContextMenu from './components/ContextMenu';
import Modal from './components/Modal';
import GameRatings, { RatingSummary } from './components/GameRatings';
import { useRatings } from './ratings/useRatings';
import ShaderTools from './components/ShaderTools';
import type { Game, Snapshot, Sort, View } from './types';

const viewNames: Record<View, string> = { library: 'Library', favorites: 'Favorites', recent: 'Recently played', hidden: 'Hidden games' };

function GameImage({ game, hero = false, shared = false, eager = false, className = '' }: { game: Game; hero?: boolean; shared?: boolean; eager?: boolean; className?: string }) {
  const [failed, setFailed] = useState(false);
  const path = hero ? game.hero || game.cover : game.cover;
  useEffect(() => setFailed(false), [path]);
  return <div className={`game-image ${className} ${failed || !path ? 'fallback-art' : ''} ${hero && !game.hero ? 'portrait-hero' : ''}`} style={{ viewTransitionName: shared ? `game-${game.id}` : undefined }}>
    {path && !failed ? <img src={imageSrc(path)} alt="" loading={hero || eager ? 'eager' : 'lazy'} decoding="async" onError={() => setFailed(true)} /> : <><Gamepad2 size={32} strokeWidth={1} /><span>{game.title}</span></>}
  </div>;
}

function GameTitle({ game, detail = false }: { game: Game; detail?: boolean }) {
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [game.logo]);
  if (game.logo && !failed) return <><h1 className="sr-only">{game.title}</h1><img className={`game-logo ${detail ? 'detail-logo' : ''}`} src={imageSrc(game.logo)} alt="" onError={() => setFailed(true)} /></>;
  return <h1 className="game-title">{game.title}</h1>;
}

function PlayButton({ game, launching, running, onPlay, compact = false }: { game: Game; launching: number | null; running: boolean; onPlay: (game: Game) => void; compact?: boolean }) {
  const busy = launching === game.id;
  const label = busy ? 'Launching…' : running ? 'Running' : game.available ? 'Play' : 'Unavailable';
  return <button className={compact ? 'quick-play' : 'primary-button play-button'} disabled={launching !== null || running || !game.available} onClick={() => onPlay(game)} aria-label={compact ? `${busy ? 'Launching' : label} ${game.title}` : undefined} title={compact ? `${label} ${game.title}` : undefined} aria-busy={busy}>
    {busy ? <LoaderCircle size={compact ? 17 : 18} className="spin" /> : running ? <Gamepad2 size={compact ? 16 : 18} /> : <Play size={compact ? 16 : 18} fill="currentColor" />}
    {!compact && <span>{label}</span>}
  </button>;
}

export default function App() {
  const [snapshot, setSnapshot] = useState<Snapshot>({ games: [], runningGameIds: [], settings: { folders: [], apiKey: '', displayName: 'Player', autoWatch: true } });
  const [loading, setLoading] = useState(true);
  const [view, setView] = useState<View>('library');
  const [search, setSearch] = useState('');
  const [genre, setGenre] = useState('all');
  const [sort, setSort] = useState<Sort>('title');
  const [layout, setLayout] = useState<'grid' | 'list'>('grid');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [detailId, setDetailId] = useState<number | null>(null);
  const [artworkId, setArtworkId] = useState<number | null>(null);
  const [editId, setEditId] = useState<number | null>(null);
  const [removeId, setRemoveId] = useState<number | null>(null);
  const [context, setContext] = useState<{ id: number; x: number; y: number } | null>(null);
  const [featuredId, setFeaturedId] = useState<number | null>(null);
  const [scanning, setScanning] = useState(false);
  const [launching, setLaunching] = useState<number | null>(null);
  const [toast, setToast] = useState<{ text: string; error: boolean } | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const mainRef = useRef<HTMLElement>(null);
  const libraryScrollRef = useRef(0);
  const launchingRef = useRef<number | null>(null);
  const runningGameIdsRef = useRef<number[]>([]);
  const refreshRequestRef = useRef(0);
  const updatingRatings = useRatings(snapshot.games, detailId);
  const refresh = useCallback(async () => {
    const request = ++refreshRequestRef.current;
    const data = await api.snapshot();
    if (request !== refreshRequestRef.current) return;
    runningGameIdsRef.current = data.runningGameIds;
    setSnapshot(data);
  }, []);
  const notify = useCallback((text: string, error = false) => setToast({ text, error }), []);
  const closeContext = useCallback(() => setContext(null), []);

  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    void (async () => {
      try {
        const fn = await api.subscribe(() => { if (active) void refresh().catch(err => { if (active) notify(String(err), true); }); });
        if (active) unsubscribe = fn; else fn();
      } catch (err) { if (active) notify(String(err), true); }
      if (!active) return;
      try { await refresh(); } catch (err) { if (active) notify(String(err), true); } finally { if (active) setLoading(false); }
    })();
    return () => { active = false; ++refreshRequestRef.current; unsubscribe?.(); };
  }, [refresh, notify]);
  useEffect(() => {
    if (!toast || toast.error) return;
    const timer = setTimeout(() => setToast(null), 6500);
    return () => clearTimeout(timer);
  }, [toast]);
  useEffect(() => {
    function keyboard(event: KeyboardEvent) {
      if (document.querySelector('dialog[open]')) return;
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); searchRef.current?.focus(); }
      if (event.key === 'Escape') { if (context) setContext(null); else if (detailId !== null) changeDetail(null); else setSearch(''); }
    }
    window.addEventListener('keydown', keyboard);
    return () => window.removeEventListener('keydown', keyboard);
  }, [context, detailId]);

  const visibleGames = snapshot.games.filter(game => !game.hidden);
  const spotlight = featuredGames(snapshot.games);
  const featuredIndex = Math.max(0, spotlight.findIndex(game => game.id === featuredId));
  const hero = spotlight[featuredIndex];
  const games = filterGames(snapshot.games, view, search, genre, sort);
  const genres = [...new Set(visibleGames.map(game => game.genre).filter(Boolean))].sort();
  const detail = snapshot.games.find(game => game.id === detailId);
  const artwork = snapshot.games.find(game => game.id === artworkId);
  const edit = snapshot.games.find(game => game.id === editId);
  const removing = snapshot.games.find(game => game.id === removeId);
  const contextGame = snapshot.games.find(game => game.id === context?.id);
  const showHero = view === 'library' && !search.trim() && genre === 'all' && !!hero;

  function navigate(next: View) { setView(next); setGenre('all'); setSearch(''); setDetailId(null); setContext(null); mainRef.current?.scrollTo({ top: 0 }); }
  function changeDetail(id: number | null) {
    if (id !== null) libraryScrollRef.current = mainRef.current?.scrollTop ?? 0;
    const update = () => {
      flushSync(() => setDetailId(id));
      mainRef.current?.scrollTo({ top: id === null ? libraryScrollRef.current : 0, behavior: 'instant' });
      if (id !== null) mainRef.current?.querySelector<HTMLButtonElement>('.back-button')?.focus({ preventScroll: true });
      else mainRef.current?.querySelector<HTMLButtonElement>(`[data-game-id="${detailId}"]`)?.focus({ preventScroll: true });
    };
    if (document.startViewTransition && !window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
      const transition = document.startViewTransition(update);
      void transition.finished.catch(() => {});
    } else update();
  }
  function openDetail(game: Game) { changeDetail(game.id); }
  async function patch(game: Game, change: Parameters<typeof api.patch>[1]) {
    try { await api.patch(game.id, change); await refresh(); } catch (err) { notify(String(err), true); }
  }
  async function launch(game: Game) {
    if (!isDesktop) { notify('Game launching is available in the Windows app.'); return; }
    if (launchingRef.current !== null || runningGameIdsRef.current.includes(game.id) || !game.available) return;
    launchingRef.current = game.id;
    ++refreshRequestRef.current;
    setLaunching(game.id);
    let started = false;
    try {
      await api.launch(game.id);
      started = true;
      ++refreshRequestRef.current;
      const runningGameIds = [...new Set([...runningGameIdsRef.current, game.id])];
      runningGameIdsRef.current = runningGameIds;
      setSnapshot(current => ({ ...current, runningGameIds }));
      await refresh();
      notify(`Starting ${game.title}…`);
    } catch (err) {
      if (!started) {
        ++refreshRequestRef.current;
        const runningGameIds = runningGameIdsRef.current.filter(id => id !== game.id);
        runningGameIdsRef.current = runningGameIds;
        setSnapshot(current => ({ ...current, runningGameIds }));
      }
      notify(String(err), true);
    } finally { launchingRef.current = null; setLaunching(null); }
  }
  async function scan() {
    if (!isDesktop || !snapshot.settings.folders.length) { setSettingsOpen(true); return; }
    setScanning(true);
    try {
      const report = await api.scan(); await refresh();
      notify(`Scan complete. ${report.added} new ${report.added === 1 ? 'game' : 'games'}, ${report.updated} updated.${report.warnings.length ? ` ${report.warnings.join(' ')}` : ''}`, report.warnings.length > 0);
    } catch (err) { notify(String(err), true); } finally { setScanning(false); }
  }
  function action(name: string, game: Game) {
    if (name === 'play') void launch(game);
    if (name === 'favorite') void patch(game, { favorite: !game.favorite });
    if (name === 'artwork') setArtworkId(game.id);
    if (name === 'edit') setEditId(game.id);
    if (name === 'hide') void patch(game, { hidden: !game.hidden });
    if (name === 'remove') setRemoveId(game.id);
    if (name === 'folder') void api.openFolder(game.id).catch(err => notify(String(err), true));
  }
  function menu(game: Game, x: number, y: number) { setContext({ id: game.id, x, y }); }
  function nextFeature(direction: number) { setFeaturedId(spotlight[(featuredIndex + direction + spotlight.length) % spotlight.length].id); }

  return <div className="app-shell">
    <a href="#main-content" className="skip-link">Skip to games</a>
    <header className="topbar">
      <button className="brand" onClick={() => navigate('library')} aria-label="Arc home"><img src="/arc-logo.svg" alt="" width="116" height="38" /></button>
      <nav className="main-nav" aria-label="Main navigation">
        {(['library', 'recent', 'favorites'] as const).map(name => <button key={name} className={view === name ? 'active' : ''} aria-current={view === name ? 'page' : undefined} onClick={() => navigate(name)}>{viewNames[name]}</button>)}
      </nav>
      <div className="topbar-actions">
        <div className="search-field"><Search size={17} /><label className="sr-only" htmlFor="library-search">Search games</label><input ref={searchRef} id="library-search" placeholder="Search games" value={search} onChange={event => { setSearch(event.target.value); setDetailId(null); }} />{search ? <button aria-label="Clear search" onClick={() => setSearch('')}><X size={15} /></button> : <kbd>Ctrl K</kbd>}</div>
        <button className="icon-button scan-button" title="Scan game folders" aria-label={scanning ? 'Scanning game folders' : 'Scan game folders'} disabled={scanning} onClick={() => void scan()}><RefreshCw size={18} className={scanning ? 'spin' : ''} /></button>
        <button className={`icon-button ${view === 'hidden' ? 'active' : ''}`} title="Hidden games" aria-label="Hidden games" aria-pressed={view === 'hidden'} onClick={() => navigate('hidden')}><EyeOff size={18} /></button>
        <button className="icon-button" title="Settings" aria-label="Settings" onClick={() => setSettingsOpen(true)}><Settings2 size={19} /></button>
      </div>
    </header>
    <main ref={mainRef} className="main-content" id="main-content" tabIndex={-1}>
      {detail ? <div className="detail-page">
        <section className="detail-hero" aria-label={detail.title}>
          <GameImage game={detail} hero /><div className="hero-shade" />
          <button className="back-button" onClick={() => changeDetail(null)}><ArrowLeft size={17} /> {viewNames[view]}</button>
          <div className="detail-content">
            {detail.genre && <span className="hero-kicker">{detail.genre}</span>}<GameTitle game={detail} detail />
            <div className="hero-actions"><PlayButton game={detail} launching={launching} running={snapshot.runningGameIds.includes(detail.id)} onPlay={game => void launch(game)} /><button className={`glass-button ${detail.favorite ? 'is-favorite' : ''}`} aria-label={detail.favorite ? 'Remove from favorites' : 'Add to favorites'} aria-pressed={detail.favorite} onClick={() => void patch(detail, { favorite: !detail.favorite })}><Heart size={19} fill={detail.favorite ? 'currentColor' : 'none'} /></button><button className="glass-button" aria-label={`Actions for ${detail.title}`} onClick={event => { const rect = event.currentTarget.getBoundingClientRect(); menu(detail, rect.left, rect.bottom + 8); }}><MoreHorizontal size={21} /></button></div>
            <div className="session-meta">{detail.lastPlayed ? <><Clock3 size={14} /><span>{lastPlayedLabel(detail.lastPlayed)}</span></> : <span>Not played yet</span>}{detail.playtime > 0 && <><span className="meta-divider" /><span>{playtimeLabel(detail.playtime)}</span></>}{!detail.available && <span className="unavailable">{detail.consoleLaunch ? 'Emulator or game unavailable' : 'Game file not found'}</span>}</div>
          </div>
          <div className="detail-poster"><GameImage game={detail} shared /></div>
        </section>
        <div className="detail-body"><GameRatings key={detail.id} game={detail} /><ShaderTools key={`shaders-${detail.id}`} game={detail} onConfigure={() => setSettingsOpen(true)} /><section className="game-information"><h2>{detail.description ? 'About the game' : 'Game details'}</h2>{detail.description && <p className="description">{detail.description}</p>}<dl><div><dt>Library status</dt><dd>{detail.hidden ? 'Hidden' : detail.available ? 'Installed' : 'Unavailable'}</dd></div>{detail.genre && <div><dt>Genre</dt><dd>{detail.genre}</dd></div>}<div><dt>Source</dt><dd>{detail.source}</dd></div><div><dt>Added to library</dt><dd>{new Date(detail.addedAt).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' })}</dd></div></dl></section><section className="installation"><h2>Manage game</h2><span className="field-label">Game folder</span><code>{detail.folder}</code><div className="manage-actions"><button className="text-button" onClick={() => action('folder', detail)}><FolderOpen size={16} /> Open folder <ArrowRight size={15} /></button><button className="text-button" onClick={() => setArtworkId(detail.id)}><Image size={16} /> Change artwork</button><button className="text-button" onClick={() => setEditId(detail.id)}><Pencil size={16} /> Game properties</button></div></section></div>
      </div> : <div className={`library-page ${showHero ? 'with-hero' : ''}`}>
        {!showHero && <h1 className="sr-only">{search.trim() ? 'Search results' : viewNames[view]}</h1>}
        {showHero && <section className="feature-hero" aria-label={`Featured game: ${hero.title}`}>
          <GameImage key={hero.id} game={hero} hero className="feature-image" /><div className="hero-shade" />
          <div className="feature-content" key={`title-${hero.id}`}><span className="hero-kicker"><span className="kicker-line" />{hero.lastPlayed ? 'Recently played' : 'From your library'}</span><GameTitle game={hero} />{hero.description && <p className="feature-description">{hero.description}</p>}<div className="hero-actions"><PlayButton game={hero} launching={launching} running={snapshot.runningGameIds.includes(hero.id)} onPlay={game => void launch(game)} /><button className="hero-details" onClick={() => openDetail(hero)}>Game details <ArrowRight size={17} /></button></div>{hero.lastPlayed && <div className="session-meta"><Clock3 size={14} /><span>{lastPlayedLabel(hero.lastPlayed)}</span>{hero.playtime > 0 && <><span className="meta-divider" /><span>{playtimeLabel(hero.playtime)}</span></>}</div>}</div>
          <div className="feature-bottom"><div className="feature-pagination"><span className="feature-number">{String(featuredIndex + 1).padStart(2, '0')}<span> / {String(spotlight.length).padStart(2, '0')}</span></span><div className="feature-progress" aria-hidden="true">{spotlight.map(game => <span className={game.id === hero.id ? 'active' : ''} key={game.id} />)}</div><div className="feature-arrows"><button className="icon-button" aria-label="Previous featured game" disabled={spotlight.length < 2} onClick={() => nextFeature(-1)}><ChevronLeft size={18} /></button><button className="icon-button" aria-label="Next featured game" disabled={spotlight.length < 2} onClick={() => nextFeature(1)}><ChevronRight size={18} /></button></div></div><div className="feature-rail" aria-label="Featured games">{spotlight.map(game => <button className={`feature-thumbnail ${game.id === hero.id ? 'active' : ''}`} aria-label={`Feature ${game.title}`} aria-pressed={game.id === hero.id} onClick={() => setFeaturedId(game.id)} title={game.title} key={game.id}><GameImage game={game} eager /></button>)}</div></div>
        </section>}
        <section className="all-games">
          <div className="library-toolbar"><div className="library-heading"><h2>{search.trim() ? 'Search results' : genre !== 'all' ? genre : view === 'library' ? 'All games' : viewNames[view]}</h2><span className="game-count">{games.length} {games.length === 1 ? 'game' : 'games'}</span></div><div className="library-options">
            {genres.length > 0 && <label className="select-wrap"><span className="sr-only">Filter by genre</span><select value={genre} onChange={event => setGenre(event.target.value)}><option value="all">All genres</option>{genres.map(item => <option key={item}>{item}</option>)}</select><ChevronDown size={13} /></label>}
            <label className="select-wrap sort-wrap"><ArrowDownWideNarrow size={16} /><span className="sr-only">Sort games</span><select value={sort} onChange={event => setSort(event.target.value as Sort)}><option value="title">Name, A–Z</option><option value="recent">Recently played</option><option value="added">Recently added</option></select><ChevronDown size={13} /></label><div className="view-switch" role="group" aria-label="Library layout"><button className={layout === 'grid' ? 'active' : ''} aria-label="Grid view" aria-pressed={layout === 'grid'} onClick={() => setLayout('grid')}><Grid2X2 size={17} /></button><button className={layout === 'list' ? 'active' : ''} aria-label="List view" aria-pressed={layout === 'list'} onClick={() => setLayout('list')}><List size={18} /></button></div><button className="add-games-button" aria-label="Add game folders" title="Add game folders" onClick={() => setSettingsOpen(true)}><Plus size={18} /><span>Add games</span></button>
          </div></div>
          {loading ? <div className="empty-state"><LoaderCircle className="spin" size={28} /><h3>Loading library…</h3></div> : games.length === 0 ? <div className="empty-state"><div className="empty-icon">{search ? <Search size={28} /> : view === 'favorites' ? <Heart size={28} /> : <Gamepad2 size={30} />}</div><h3>{search.trim() ? 'No matching games' : genre !== 'all' ? 'No games in this genre' : view === 'favorites' ? 'No favorites yet' : view === 'hidden' ? 'No hidden games' : view === 'recent' ? 'No recently played games' : 'Your library is empty'}</h3><p>{search.trim() || genre !== 'all' ? 'Try another search or clear the filters.' : view === 'favorites' ? 'Use a game’s menu to add it to favorites.' : view === 'hidden' ? 'Games you hide will appear here.' : view === 'recent' ? 'Games launched through Arc will appear here.' : 'Add a game folder, then scan it to build your library.'}</p>{search.trim() || genre !== 'all' ? <button className="secondary-button" onClick={() => { setSearch(''); setGenre('all'); }}>Clear filters</button> : view === 'library' && <button className="primary-button" onClick={() => setSettingsOpen(true)}><Plus size={17} /> Add game folder</button>}</div> : <div className={`game-grid ${layout === 'list' ? 'list-layout' : ''}`}>
            {games.map((game, index) => <article className={`game-card ${!game.available ? 'unavailable-card' : ''}`} key={game.id} onContextMenu={event => { event.preventDefault(); menu(game, event.clientX, event.clientY); }}>
              <div className="cover-wrap"><button className="cover-button" data-game-id={game.id} aria-label={`View ${game.title}`} onClick={() => openDetail(game)}><GameImage game={game} shared eager={index < 8} /></button><div className="cover-actions">{!game.hidden && game.available && <PlayButton game={game} launching={launching} running={snapshot.runningGameIds.includes(game.id)} onPlay={item => void launch(item)} compact />}{!game.cover && <button className="quick-artwork" aria-label={`Choose artwork for ${game.title}`} onClick={() => setArtworkId(game.id)}><Image size={17} /></button>}</div></div>
              <div className="card-meta"><button className="card-title" title={game.title} onClick={() => openDetail(game)}>{game.favorite && <Heart size={12} fill="currentColor" aria-label="Favorite" />}<span>{game.title}</span></button><RatingSummary game={game} />{layout === 'list' && <span className="card-description">{[game.genre, game.lastPlayed ? lastPlayedLabel(game.lastPlayed) : null, game.playtime > 0 ? playtimeLabel(game.playtime) : null].filter(Boolean).join(' · ')}</span>}{!game.available && <span className="card-warning">{game.consoleLaunch ? 'Emulator or game unavailable' : 'Game file not found'}</span>}{game.hidden && <button className="restore-button" onClick={() => void patch(game, { hidden: false })}>Show in library <ArrowRight size={12} /></button>}</div>
              <div className="card-controls">{layout === 'list' && !game.hidden && <PlayButton game={game} launching={launching} running={snapshot.runningGameIds.includes(game.id)} onPlay={item => void launch(item)} compact />}<button className="card-menu" aria-label={`Actions for ${game.title}`} onClick={event => { const rect = event.currentTarget.getBoundingClientRect(); menu(game, rect.right - 240, rect.bottom + 5); }}><MoreHorizontal size={20} /></button></div>
            </article>)}
          </div>}
          <footer className="content-footer"><span>{isDesktop ? `${snapshot.settings.folders.length} game folders` : 'Preview library'}</span><button onClick={() => void scan()} disabled={scanning}><RefreshCw size={14} className={scanning ? 'spin' : ''} />{scanning ? 'Scanning…' : 'Scan folders'}</button><span className="app-version">{updatingRatings ? 'Updating ratings… · ' : ''}Arc 0.1.11</span></footer>
        </section>
      </div>}
    </main>
    {settingsOpen && <Settings settings={snapshot.settings} onClose={() => setSettingsOpen(false)} onSave={async settings => { await api.settings(settings); await refresh(); notify('Settings saved.'); }} />}
    {artwork && <ArtworkPicker game={artwork} onClose={() => setArtworkId(null)} onChange={refresh} />}
    {edit && <EditGame game={edit} onClose={() => setEditId(null)} onChange={refresh} />}
    {removing && <Modal title={`Remove ${removing.title}?`} onClose={() => setRemoveId(null)}><p className="modal-intro">Remove this game from the library and skip it in future scans. The game files stay on your device.</p><div className="modal-footer"><button className="text-button" onClick={() => setRemoveId(null)}>Cancel</button><button className="danger-button" onClick={() => void api.remove(removing.id).then(async () => { setRemoveId(null); if (detailId === removing.id) setDetailId(null); await refresh(); }).catch(err => notify(String(err), true))}>Remove from library</button></div></Modal>}
    {context && contextGame && <ContextMenu game={contextGame} playDisabled={launching !== null || snapshot.runningGameIds.includes(contextGame.id) || !contextGame.available} playLabel={launching === contextGame.id ? 'Launching…' : snapshot.runningGameIds.includes(contextGame.id) ? 'Running' : contextGame.available ? 'Play' : 'Unavailable'} x={context.x} y={context.y} onAction={action} onClose={closeContext} />}
    {toast && <div className={`toast ${toast.error ? 'error' : ''}`} role={toast.error ? 'alert' : 'status'}>{toast.error ? <X size={18} /> : <Check size={18} />}<span>{toast.text}</span><button aria-label="Dismiss notification" onClick={() => setToast(null)}><X size={16} /></button></div>}
  </div>;
}
