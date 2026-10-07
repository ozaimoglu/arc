import { useCallback, useEffect, useRef, useState } from 'react';
import { Cpu, RefreshCw, Square, ArrowUpRight, Trash2, CircleDot } from 'lucide-react';
import { api, isDesktop } from '../bridge';
import type { Game, ShaderSnapshot } from '../types';
import Modal from './Modal';

const labels: Record<string, string> = { Ready: 'Ready to compile', Stale: 'Refresh needed', Warmed: 'Shaders prepared', NeedsRecording: 'Recording needed', Unsupported: 'Not supported', Unknown: 'Analyze again' };
const phases: Record<string, string> = { Analyzing: 'Analyzing game shaders', Waiting: 'Waiting to compile', Indexing: 'Reading shaders', Planning: 'Building a shader plan', Materializing: 'Preparing pipelines', Warming: 'Compiling shaders', Refreshing: 'Checking cache status', Clearing: 'Clearing shader cache', CacheCleared: 'Shader cache cleared', Paused: 'Paused', Stopping: 'Stopping after current compiles finish', Done: 'Finished', Failed: 'Could not finish', Stopped: 'Stopped' };

export default function ShaderTools({ game, onConfigure }: { game: Game; onConfigure: () => void }) {
  const [snapshot, setSnapshot] = useState<ShaderSnapshot | null>(null);
  const [error, setError] = useState('');
  const [loadError, setLoadError] = useState('');
  const [pending, setPending] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);
  const [confirmRecord, setConfirmRecord] = useState(false);
  const [gamePrecache, setGamePrecache] = useState(false);
  const mounted = useRef(false);
  const refresh = useCallback(async () => {
    try { const value = await api.shaderState(game.id); if (mounted.current) { setSnapshot(value); setLoadError(''); } }
    catch (err) { if (mounted.current) setLoadError(String(err)); }
  }, [game.id]);
  useEffect(() => {
    mounted.current = true;
    if (!game.consoleLaunch && isDesktop) void refresh();
    return () => { mounted.current = false; };
  }, [game.consoleLaunch, refresh]);
  useEffect(() => {
    if (!isDesktop || game.consoleLaunch) return;
    // Poll backend-owned state so navigation/remounting never loses an in-flight job.
    const timer = setInterval(() => void refresh(), snapshot?.busy ? 1200 : 5000);
    return () => clearInterval(timer);
  }, [snapshot?.busy, game.consoleLaunch, refresh]);

  const job = snapshot?.job?.gameId === game.id || snapshot?.job?.running ? snapshot.job : null;
  async function start(action: 'analyze' | 'compile' | 'prepareRecording') {
    setConfirmRecord(false);
    setPending(true); setError('');
    try { await api.startShaderJob(game.id, action); await refresh(); }
    catch (err) { if (mounted.current) setError(String(err)); }
    finally { if (mounted.current) setPending(false); }
  }
  async function stop() {
    if (!job) return;
    setStopping(true); setError('');
    try { await api.stopShaderJob(job.gameId); await refresh(); }
    catch (err) { if (mounted.current) setError(String(err)); }
    finally { if (mounted.current) setStopping(false); }
  }
  async function clear() {
    setConfirmClear(false); setPending(true); setError('');
    try { await api.clearShaderCache(game.id, gamePrecache); await refresh(); }
    catch (err) { if (mounted.current) setError(String(err)); }
    finally { if (mounted.current) setPending(false); }
  }

  if (game.consoleLaunch) return <section className="shader-tools"><h2>Shader preparation</h2><p className="field-help">SCSKiller prepares native PC DirectX shaders. shadPS4 manages its own shader cache.</p></section>;
  if (!isDesktop) return <section className="shader-tools"><h2>Shader preparation</h2><p className="field-help">SCSKiller integration is available in the Windows app.</p></section>;
  const status = snapshot?.game;
  const needsFolderConfirmation = status?.status === 'Unsupported' && status.reason.includes('once you confirm its game folder');
  const locked = pending || !snapshot || snapshot.busy || !game.available;
  return <section className="shader-tools" aria-labelledby="shader-heading">
    <div className="shader-heading"><h2 id="shader-heading"><Cpu size={18} aria-hidden="true" /> Shader preparation</h2><span className="shader-provider">SCSKiller</span></div>
    {!snapshot?.installed ? <>
      <p className="field-help">Prepare supported game shaders before you play. Connect your SCSKiller installation to get started.</p>
      <button className="secondary-button" onClick={onConfigure}>Connect SCSKiller <ArrowUpRight size={15} aria-hidden="true" /></button>
    </> : <>
      <div className={`shader-status ${status?.status === 'Warmed' ? 'prepared' : ''}`}><span>{needsFolderConfirmation ? 'Folder confirmation needed' : status ? labels[status.status] ?? 'Analyze again' : 'Not analyzed'}</span>{status?.engine && <small>{[status.engine, status.graphicsApi].filter(Boolean).join(' · ')}</small>}</div>
      <p className="field-help">{status?.reason ?? 'Analyze this installation to check shader support. Arc adds its local executable to SCSKiller when needed.'}</p>
      {snapshot.warning && <p className="form-error" role="alert">{snapshot.warning}</p>}
      {status?.warmedAt && <p className="shader-last">Last prepared {new Date(status.warmedAt).toLocaleString()}{status.driver ? ` · Driver ${status.driver}` : ''}</p>}
      {(status?.status === 'NeedsRecording' || needsFolderConfirmation) && !status?.recorderInstalled && <p className="field-help">{snapshot.recordingSupported && status?.canRecord ? 'Prepare a recording, play for at least five minutes in the game world, then close the game and analyze again.' : 'Confirm this game’s folder in SCSKiller, create a gameplay recording, then analyze again. Arc recording setup requires the compatibility CLI.'}</p>}
      {status?.recorderInstalled && <div className="shader-recording" role="status"><strong><CircleDot size={15} aria-hidden="true" /> Recorder ready</strong><p className="field-help">{status.recordedEnough ? 'Gameplay recording captured. Close the game and analyze again to check compilation.' : 'Play for at least five minutes in the game world, then close the game and analyze again. Menus alone may not capture the required pipelines.'}</p><small>At last analysis: {((status.recordingBytes ?? 0) / 1048576).toFixed(1)} MB recorded</small></div>}
      {status?.recorderNote && !status.recorderInstalled && status.canRecord && !needsFolderConfirmation && <p className="field-help">{status.recorderNote}</p>}
      {snapshot.recordingSupported && status?.canRecord && !status.recorderInstalled && <button className="secondary-button" disabled={locked} onClick={() => setConfirmRecord(true)}><CircleDot size={15} aria-hidden="true" /> Prepare recording</button>}
      <div className="shader-actions"><button className="secondary-button" disabled={locked} onClick={() => void start('analyze')}><RefreshCw size={15} aria-hidden="true" /> {status ? 'Analyze again' : 'Analyze game'}</button><button className="primary-button" disabled={locked || !status?.canCompile} onClick={() => void start('compile')}><Cpu size={16} aria-hidden="true" /> {status?.status === 'Warmed' ? 'Recompile shaders' : 'Compile shaders'}</button><button className="text-button" disabled={locked || !status} onClick={() => { setGamePrecache(false); setConfirmClear(true); }}><Trash2 size={15} aria-hidden="true" /> Clear cache</button></div>
      {!game.available && <p className="field-help">Restore this game’s installation before analyzing.</p>}
      {snapshot.busy && !job?.running && <p className="field-help" role="status">The shader cache limit is being changed. Wait for it to finish before preparing or clearing shaders.</p>}
    </>}
    {job && <div className="shader-job">
      <div className="shader-job-heading"><span role="status" aria-atomic="true">{job.phase === 'PreparingRecording' ? 'Installing gameplay recorder' : phases[job.phase] ?? 'Preparing shaders'}{job.gameId !== game.id ? ` · ${job.title}` : ''}</span>{job.running && !['clearCache', 'prepareRecording'].includes(job.action) && <button className="text-button" disabled={stopping || job.phase === 'Stopping'} onClick={() => void stop()}><Square size={13} aria-hidden="true" /> Stop</button>}</div>
      {job.running && <progress aria-label="Shader preparation progress" />}
      {job.running && job.lines.length > 0 && <p className="shader-live-line">{job.lines.at(-1)}</p>}
      {job.error && <p className="form-error" role="alert">{job.error}</p>}
      {job.lines.length > 0 && <details className="shader-output"><summary>Operation output</summary><pre tabIndex={0} role="region" aria-label="SCSKiller operation output">{job.lines.join('\n')}</pre></details>}
    </div>}
    {(error || loadError) && <p className="form-error" role="alert">{error || loadError}</p>}
    {confirmRecord && <Modal title="Prepare gameplay recording?" onClose={() => setConfirmRecord(false)}>
      <p className="modal-intro">Prepare SCSKiller’s recorder for <strong>{game.title}</strong>. This confirms this installation’s own game folder and adds a recorder DLL beside the executable.</p>
      <p className="field-help">Keep the game closed during setup. SCSKiller checks anti-cheat, existing DLLs and running processes. Play for at least five minutes afterward, then close the game and analyze again. You can remove the recorder in SCSKiller.</p>
      <div className="modal-footer"><button className="secondary-button" autoFocus onClick={() => setConfirmRecord(false)}>Cancel</button><button className="primary-button" disabled={locked || !status?.canRecord} onClick={() => void start('prepareRecording')}><CircleDot size={15} aria-hidden="true" /> Install recorder</button></div>
    </Modal>}
    {confirmClear && <Modal title="Clear shader cache?" onClose={() => setConfirmClear(false)}>
      <p className="modal-intro">Clear the driver and Windows shader cache files SCSKiller can attribute to <strong>{game.title}</strong>. The next launch may stutter while shaders rebuild. Game files and saves are preserved.</p>
      <label className="cache-precache"><input type="checkbox" checked={gamePrecache} onChange={event => setGamePrecache(event.target.checked)} /><span>Also clear the game’s generated precache<small>Includes detected Unreal user pipeline and shader precache files. Shipped shader libraries are preserved.</small></span></label>
      <p className="field-help">Keep the game closed. SCSKiller refuses to delete cache shared with another game or files that are in use.</p>
      <div className="modal-footer"><button className="secondary-button" autoFocus onClick={() => setConfirmClear(false)}>Keep cache</button><button className="danger-button" disabled={locked || !status} onClick={() => void clear()}><Trash2 size={15} aria-hidden="true" /> Clear shader cache</button></div>
    </Modal>}
  </section>;
}
