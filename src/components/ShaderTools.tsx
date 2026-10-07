import { useCallback, useEffect, useRef, useState } from 'react';
import { Cpu, RefreshCw, Square, ArrowUpRight } from 'lucide-react';
import { api, isDesktop } from '../bridge';
import type { Game, ShaderSnapshot } from '../types';

const labels: Record<string, string> = { Ready: 'Ready to compile', Stale: 'Refresh needed', Warmed: 'Shaders prepared', NeedsRecording: 'Recording needed', Unsupported: 'Not supported', Unknown: 'Analyze again' };
const phases: Record<string, string> = { Analyzing: 'Analyzing game shaders', Waiting: 'Waiting to compile', Indexing: 'Reading shaders', Planning: 'Building a shader plan', Materializing: 'Preparing pipelines', Warming: 'Compiling shaders', Refreshing: 'Checking compiled cache', Paused: 'Paused', Stopping: 'Stopping after current compiles finish', Done: 'Finished', Failed: 'Could not finish', Stopped: 'Stopped' };

export default function ShaderTools({ game, onConfigure }: { game: Game; onConfigure: () => void }) {
  const [snapshot, setSnapshot] = useState<ShaderSnapshot | null>(null);
  const [error, setError] = useState('');
  const [loadError, setLoadError] = useState('');
  const [pending, setPending] = useState(false);
  const [stopping, setStopping] = useState(false);
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
  async function start(action: 'analyze' | 'compile') {
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
      {status?.status === 'NeedsRecording' && <p className="field-help">Create a recording in SCSKiller, then analyze again. Arc does not install a recorder or bypass anti-cheat.</p>}
      {needsFolderConfirmation && <p className="field-help">Confirm this game’s folder in SCSKiller, then create a recording and analyze again.</p>}
      <div className="shader-actions"><button className="secondary-button" disabled={locked} onClick={() => void start('analyze')}><RefreshCw size={15} aria-hidden="true" /> {status ? 'Analyze again' : 'Analyze game'}</button><button className="primary-button" disabled={locked || !status?.canCompile} onClick={() => void start('compile')}><Cpu size={16} aria-hidden="true" /> {status?.status === 'Warmed' ? 'Recompile shaders' : 'Compile shaders'}</button></div>
      {!game.available && <p className="field-help">Restore this game’s installation before analyzing.</p>}
    </>}
    {job && <div className="shader-job">
      <div className="shader-job-heading"><span role="status" aria-atomic="true">{phases[job.phase] ?? 'Preparing shaders'}{job.gameId !== game.id ? ` · ${job.title}` : ''}</span>{job.running && <button className="text-button" disabled={stopping || job.phase === 'Stopping'} onClick={() => void stop()}><Square size={13} aria-hidden="true" /> Stop</button>}</div>
      {job.running && <progress aria-label="Shader preparation progress" />}
      {job.running && job.lines.length > 0 && <p className="shader-live-line">{job.lines.at(-1)}</p>}
      {job.error && <p className="form-error" role="alert">{job.error}</p>}
      {job.lines.length > 0 && <details className="shader-output"><summary>Operation output</summary><pre tabIndex={0} role="region" aria-label="SCSKiller operation output">{job.lines.join('\n')}</pre></details>}
    </div>}
    {(error || loadError) && <p className="form-error" role="alert">{error || loadError}</p>}
  </section>;
}
