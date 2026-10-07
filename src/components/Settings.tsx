import { useState } from 'react';
import { Folder, Plus, Trash2, Eye, EyeOff, ArrowUpRight, Check, Radio } from 'lucide-react';
import Modal from './Modal';
import { api, isDesktop } from '../bridge';
import type { Settings as SettingsType } from '../types';

export default function Settings({ settings, onClose, onSave }: { settings: SettingsType; onClose: () => void; onSave: (settings: SettingsType) => Promise<void> }) {
  const [draft, setDraft] = useState({ ...settings, folders: [...settings.folders] });
  const [path, setPath] = useState('');
  const [reveal, setReveal] = useState(false);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  async function addFolder() {
    try {
      const selected = isDesktop ? await api.pickFolder() : path.trim();
      if (selected && !draft.folders.some(folder => folder.toLowerCase() === selected.toLowerCase())) {
        setDraft({ ...draft, folders: [...draft.folders, selected] }); setPath('');
      }
    } catch (err) { setError(String(err)); }
  }
  async function submit() {
    setBusy(true); setError('');
    try { await onSave(draft); onClose(); } catch (err) { setError(String(err)); } finally { setBusy(false); }
  }
  async function chooseShaderTool() {
    setError('');
    try { const selected = await api.pickShaderTool(); if (selected) setDraft(previous => ({ ...previous, shaderTool: selected })); }
    catch (err) { setError(String(err)); }
  }
  return <Modal title="Settings" onClose={onClose}>
    <p className="modal-intro">Manage game folders, artwork and your profile.</p>
    <label className="field-label" htmlFor="display-name">Your name</label>
    <input id="display-name" value={draft.displayName} maxLength={40} onChange={event => setDraft({ ...draft, displayName: event.target.value })} />
    <div className="setting-heading"><label className="field-label">Game folders</label><span>{draft.folders.length} added</span></div>
    <div className="folder-list">{draft.folders.map(folder => <div className="folder-row" key={folder}><Folder size={18} /><span title={folder}>{folder}</span><button className="icon-button" aria-label={`Remove ${folder}`} onClick={() => setDraft({ ...draft, folders: draft.folders.filter(item => item !== folder) })}><Trash2 size={16} /></button></div>)}{draft.folders.length === 0 && <p className="folder-empty">Add a folder like D:\Games to get started.</p>}</div>
    {!isDesktop && <><label className="field-label" htmlFor="folder-path">Preview folder path</label><input id="folder-path" placeholder="D:\Games" value={path} onChange={event => setPath(event.target.value)} /><p className="field-help">Folder scanning is available in the desktop app.</p></>}
    <button className="secondary-button add-folder" onClick={() => void addFolder()}><Plus size={17} /> Add game folder</button>
    <div className="setting-heading"><label className="field-label" htmlFor="api-key">SteamGridDB API key</label><a href="https://www.steamgriddb.com/profile/preferences/api" target="_blank" rel="noreferrer" onClick={event => { if (isDesktop) { event.preventDefault(); void api.openArtworkSite().catch(err => setError(String(err))); } }}>Get a key <ArrowUpRight size={13} /></a></div>
    <div className="password-field"><input id="api-key" disabled={!isDesktop} autoComplete="off" type={reveal ? 'text' : 'password'} placeholder={isDesktop ? 'Paste your personal API key' : 'Available in the desktop app'} value={draft.apiKey} onChange={event => setDraft({ ...draft, apiKey: event.target.value.trim() })} /><button className="icon-button" aria-label={reveal ? 'Hide API key' : 'Show API key'} onClick={() => setReveal(!reveal)}>{reveal ? <EyeOff size={18} /> : <Eye size={18} />}</button></div>
    <p className="field-help">{isDesktop ? 'Required for online artwork search.' : 'Connect SteamGridDB in the desktop app.'}</p>
    <div className="setting-heading"><label className="field-label" htmlFor="shader-tool">SCSKiller · Shader preparation</label><button className="text-button" disabled={!isDesktop} onClick={() => void api.openShaderSite().catch(err => setError(String(err)))}>Download <ArrowUpRight size={13} aria-hidden="true" /></button></div>
    <input id="shader-tool" readOnly value={draft.shaderTool ?? ''} placeholder="Optional · choose your SCSKiller installation" />
    <div className="shader-settings-actions"><button className="secondary-button" disabled={!isDesktop} onClick={() => void chooseShaderTool()}><Folder size={16} aria-hidden="true" /> Choose SCSKiller</button>{draft.shaderTool && <button className="text-button" onClick={() => setDraft({ ...draft, shaderTool: '' })}>Reset</button>}</div>
    <p className="field-help">Use the official SCSKiller app or portable folder. Analyze and compile native DirectX games from their detail page. Recording stays in SCSKiller.</p>
    <label className="toggle-row"><span><Radio size={18} /><span>Keep my library up to date<small>Watch added folders for new and removed games.</small></span></span><input type="checkbox" checked={draft.autoWatch} onChange={event => setDraft({ ...draft, autoWatch: event.target.checked })} /><span className="switch" /></label>
    {error && <p className="form-error" role="alert">{error}</p>}
    <div className="modal-footer"><button className="text-button" onClick={onClose}>Cancel</button><button className="primary-button" onClick={() => void submit()} disabled={busy}>{busy ? 'Saving…' : <><Check size={17} /> Save changes</>}</button></div>
  </Modal>;
}
