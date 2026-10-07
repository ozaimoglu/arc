import { useCallback, useEffect, useRef, useState } from 'react';
import { RefreshCw, ShieldCheck } from 'lucide-react';
import { api, isDesktop } from '../bridge';
import type { ShaderCacheLimit, ShaderCacheState } from '../types';

const sizes: { value: ShaderCacheLimit; label: string }[] = [
  { value: 'default', label: 'Driver default' }, ...(['1', '5', '10', '20', '50', '100'] as const).map(value => ({ value, label: `${value} GB` })),
  { value: 'unlimited', label: 'Unlimited' },
];

export default function ShaderCacheSettings({ connectionChanged = false }: { connectionChanged?: boolean }) {
  const [state, setState] = useState<ShaderCacheState | null>(null);
  const [selected, setSelected] = useState<ShaderCacheLimit | ''>('');
  const [loading, setLoading] = useState(true);
  const [applying, setApplying] = useState(false);
  const [error, setError] = useState('');
  const [loadError, setLoadError] = useState('');
  const [message, setMessage] = useState('');
  const mounted = useRef(false);
  const request = useRef(0);
  const refresh = useCallback(async () => {
    const current = ++request.current;
    if (mounted.current) setLoading(true);
    try {
      const value = await api.shaderCacheState();
      if (mounted.current && current === request.current) { setState(value); setSelected(value.selectedLimit ?? ''); setLoadError(''); }
    } catch (err) { if (mounted.current && current === request.current) setLoadError(String(err)); }
    finally { if (mounted.current && current === request.current) setLoading(false); }
  }, []);
  useEffect(() => {
    mounted.current = true; if (isDesktop) void refresh();
    return () => { mounted.current = false; request.current++; };
  }, [refresh]);
  useEffect(() => {
    if (!state?.busy || applying) return;
    const timer = setInterval(() => void refresh(), 5000);
    return () => clearInterval(timer);
  }, [state?.busy, applying, refresh]);
  async function apply() {
    if (!selected) return;
    setApplying(true); setError(''); setMessage('');
    try {
      const result = await api.setShaderCacheLimit(selected);
      if (mounted.current) setMessage(result);
      await refresh();
    } catch (err) { if (mounted.current) setError(String(err)); await refresh(); }
    finally { if (mounted.current) setApplying(false); }
  }
  if (!isDesktop) return null;
  const locked = loading || applying || state?.busy || connectionChanged || !state?.configurable || Boolean(loadError);
  return <section className="shader-cache-settings" aria-labelledby="cache-settings-heading">
    <div className="shader-heading"><h3 id="cache-settings-heading">Shader cache</h3><button className="text-button" disabled={loading || applying} onClick={() => void refresh()}><RefreshCw size={13} aria-hidden="true" /> Refresh</button></div>
    {loading && !state ? <p className="field-help" role="status">Reading driver cache…</p> : state?.installed ? <>
      <dl className="cache-readings"><div><dt>On disk</dt><dd>{state.usage}</dd></div><div><dt>Current limit</dt><dd>{state.limit}</dd></div></dl>
      <p className="field-help cache-gpu">{state.gpu}</p>
      {state.configurable ? <>
        <label className="field-label" htmlFor="shader-cache-size">Maximum cache size</label>
        <div className="cache-size-controls"><select id="shader-cache-size" disabled={locked} value={selected} onChange={event => { setSelected(event.target.value as ShaderCacheLimit); setMessage(''); }}>
          {!state.selectedLimit && <option value="" disabled>Current: {state.limit}</option>}
          {sizes.map(size => <option key={size.value} value={size.value}>{size.label}</option>)}
        </select><button className="secondary-button" disabled={locked || !selected || selected === state.selectedLimit} onClick={() => void apply()}><ShieldCheck size={15} aria-hidden="true" /> {applying ? 'Awaiting Windows approval…' : 'Apply limit'}</button></div>
        <p className="field-help">Applies immediately to the NVIDIA driver for all games. Close running games first. Windows will ask for administrator approval. A smaller limit may evict older shaders.</p>
      </> : <p className="field-help">SCSKiller cannot change the cache limit on this GPU.</p>}
      <p className="field-help">To clear a game’s shader cache, open its details and choose Clear cache.</p>
      {state.busy && !applying && <p className="field-help" role="status">A shader operation is running. Cache size changes are locked until it finishes.</p>}
    </> : !loadError && <p className="field-help">Connect SCSKiller and save Settings to manage shader caches.</p>}
    {connectionChanged && <p className="field-help">Save your SCSKiller connection before changing cache settings.</p>}
    {message && <p className="cache-success" role="status">{message}</p>}
    {(error || loadError) && <p className="form-error" role="alert">{error || loadError}</p>}
  </section>;
}
