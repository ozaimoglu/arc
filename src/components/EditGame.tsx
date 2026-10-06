import { useState } from 'react';
import { Check } from 'lucide-react';
import Modal from './Modal';
import { api } from '../bridge';
import type { Game } from '../types';

export default function EditGame({ game, onClose, onChange }: { game: Game; onClose: () => void; onChange: () => Promise<void> }) {
  const [title, setTitle] = useState(game.title);
  const [genre, setGenre] = useState(game.genre);
  const [description, setDescription] = useState(game.description);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  async function save() {
    if (!title.trim()) { setError('Give your game a name.'); return; }
    setBusy(true);
    try { await api.patch(game.id, { title: title.trim(), genre: genre.trim(), description }); await onChange(); onClose(); } catch (err) { setError(String(err)); } finally { setBusy(false); }
  }
  return <Modal title="Game properties" onClose={onClose}>
    <label className="field-label" htmlFor="game-name">Game name</label><input id="game-name" value={title} maxLength={200} onChange={event => setTitle(event.target.value)} />
    <label className="field-label" htmlFor="game-genre">Genre</label><input id="game-genre" value={genre} maxLength={60} onChange={event => setGenre(event.target.value)} />
    <label className="field-label" htmlFor="game-description">Description</label><textarea id="game-description" value={description} maxLength={2000} rows={4} onChange={event => setDescription(event.target.value)} />
    <div className="property-box"><span>{game.consoleLaunch ? 'PS4 game file' : 'Executable'}</span><code>{game.exePath}</code></div>
    {game.consoleLaunch && <div className="property-box"><span>Platform</span><span>{game.source} · {game.consoleLaunch.titleId}</span></div>}
    {error && <p className="form-error" role="alert">{error}</p>}
    <div className="modal-footer"><button className="text-button" onClick={onClose}>Cancel</button><button className="primary-button" onClick={() => void save()} disabled={busy}><Check size={17} /> {busy ? 'Saving…' : 'Save changes'}</button></div>
  </Modal>;
}
