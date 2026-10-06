import { useEffect, useState } from 'react';
import { ImagePlus, Search, Check, LoaderCircle } from 'lucide-react';
import Modal from './Modal';
import { api, isDesktop } from '../bridge';
import type { Artwork, ArtworkKind, Game, GameMatch } from '../types';

export default function ArtworkPicker({ game, onClose, onChange }: { game: Game; onClose: () => void; onChange: () => Promise<void> }) {
  const [kind, setKind] = useState<ArtworkKind>('grid');
  const [items, setItems] = useState<Artwork[]>([]);
  const [query, setQuery] = useState(game.title);
  const [matches, setMatches] = useState<GameMatch[]>([]);
  const [linked, setLinked] = useState(Boolean(game.sgdbId));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  useEffect(() => {
    let active = true;
    if (!isDesktop || !linked) return;
    setBusy(true); setError('');
    api.artworks(game.id, kind).then(data => { if (active) setItems(data); }).catch(err => { if (active) setError(String(err)); }).finally(() => { if (active) setBusy(false); });
    return () => { active = false; };
  }, [game.id, game.sgdbId, kind, linked]);
  async function search() {
    setBusy(true); setError('');
    try { setMatches(await api.searchMetadata(query)); } catch (err) { setError(String(err)); } finally { setBusy(false); }
  }
  async function link(match: GameMatch) {
    setBusy(true); setError('');
    try { await api.linkMetadata(game.id, match.id); await onChange(); setLinked(true); setMatches([]); } catch (err) { setError(String(err)); } finally { setBusy(false); }
  }
  async function choose(id: number) {
    setBusy(true); setError('');
    try { await api.artwork(game.id, kind, id); await onChange(); onClose(); } catch (err) { setError(String(err)); } finally { setBusy(false); }
  }
  async function upload() {
    setBusy(true); setError('');
    try { if (await api.localArtwork(game.id, kind)) { await onChange(); onClose(); } } catch (err) { setError(String(err)); } finally { setBusy(false); }
  }
  return <Modal title={`Artwork for ${game.title}`} onClose={onClose} wide>
    <p className="modal-intro">Choose artwork from SteamGridDB or import an image.</p>
    <div className="art-toolbar"><div className="segmented">{(['grid', 'hero', 'logo'] as const).map(type => <button aria-pressed={kind === type} className={kind === type ? 'active' : ''} onClick={() => setKind(type)} key={type}>{type === 'grid' ? 'Cover' : type === 'hero' ? 'Hero' : 'Logo'}</button>)}</div><button className="secondary-button" onClick={() => void upload()} disabled={!isDesktop || busy}><ImagePlus size={17} /> From my device</button></div>
    {!isDesktop ? <div className="art-empty"><ImagePlus size={36} /><h3>Artwork selection</h3><p>Open the Windows app to search SteamGridDB or import a local image.</p></div> : <>
      <form className="metadata-search" onSubmit={event => { event.preventDefault(); void search(); }}><label className="sr-only" htmlFor="metadata-query">Search SteamGridDB</label><input id="metadata-query" value={query} onChange={event => setQuery(event.target.value)} /><button className="secondary-button" disabled={busy || !query.trim()}><Search size={16} /> Find game</button></form>
      {matches.map(match => <button className="match-row" key={match.id} onClick={() => void link(match)} disabled={busy}><span>{match.name}{match.releaseDate ? ` (${new Date(match.releaseDate * 1000).getUTCFullYear()})` : ''}</span><Check size={16} aria-hidden="true" /></button>)}
      {!linked && !busy && <p className="field-help">Find and select the correct game above to browse its artwork.</p>}
      {busy && <div className="art-empty"><LoaderCircle className="spin" size={24} /><p>Finding your artwork…</p></div>}
      {!busy && <div className={`art-grid ${kind}`} >{items.map(item => <button key={item.id} onClick={() => void choose(item.id)}><img src={item.thumb || item.url} alt={`Artwork by ${item.author}`} loading="lazy" /><span>by {item.author}</span></button>)}</div>}
      {linked && !busy && items.length === 0 && !error && <p className="field-help">No artwork found for this type. Try a different type or import a local image.</p>}
    </>}
    {error && <p className="form-error" role="alert">{error}</p>}
  </Modal>;
}
