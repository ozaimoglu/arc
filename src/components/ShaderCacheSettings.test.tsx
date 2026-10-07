// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { ShaderCacheState } from '../types';
const bridge = vi.hoisted(() => ({ shaderCacheState: vi.fn(), setShaderCacheLimit: vi.fn() }));
vi.mock('../bridge', () => ({ isDesktop: true, api: bridge }));
import ShaderCacheSettings from './ShaderCacheSettings';
let root: Root; let host: HTMLDivElement; let state: ShaderCacheState;
beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
  state = { installed: true, gpu: 'RTX 4090 (Nvidia)', usage: 'Up to 51,5 GB', limit: '100 GB', selectedLimit: '100', configurable: true, busy: false };
  bridge.shaderCacheState.mockImplementation(async () => structuredClone(state));
  bridge.setShaderCacheLimit.mockImplementation(async (limit: string) => { state.selectedLimit = limit as ShaderCacheState['selectedLimit']; state.limit = `${limit} GB`; return 'Shader cache limit applied.'; });
  host = document.createElement('div'); document.body.append(host); root = createRoot(host);
});
afterEach(async () => { await act(async () => root.unmount()); host.remove(); vi.clearAllMocks(); vi.useRealTimers(); vi.unstubAllGlobals(); });
async function render(connectionChanged = false) { await act(async () => root.render(<ShaderCacheSettings connectionChanged={connectionChanged} />)); }
function button(text: string) { return Array.from(host.querySelectorAll('button')).find(button => button.textContent?.includes(text))!; }
async function choose(value: string) { await act(async () => { const select = host.querySelector('select')!; select.value = value; select.dispatchEvent(new Event('change', { bubbles: true })); }); }
it('reads actual driver values and applies a selected size independently of Save Settings', async () => {
  await render(); expect(host.textContent).toContain('Up to 51,5 GB'); expect(host.textContent).toContain('100 GB');
  expect(button('Apply limit').disabled).toBe(true);
  await choose('20'); await act(async () => button('Apply limit').click());
  expect(bridge.setShaderCacheLimit).toHaveBeenCalledWith('20'); expect(host.textContent).toContain('Shader cache limit applied.');
  expect(host.querySelector('select')?.value).toBe('20'); expect(button('Apply limit').disabled).toBe(true);
});
it('offers driver default and unlimited without silently changing the existing limit', async () => {
  await render(); const values = Array.from(host.querySelectorAll('option')).map(option => option.value);
  expect(values).toContain('default'); expect(values).toContain('unlimited');
  await choose('default'); expect(bridge.setShaderCacheLimit).not.toHaveBeenCalled();
});
it('does not invent a selectable value for a custom driver setting', async () => {
  state.limit = '16 GB'; state.selectedLimit = null;
  await render(); expect(host.querySelector('select')?.value).toBe(''); expect(button('Apply limit').disabled).toBe(true);
  await choose('50'); expect(button('Apply limit').disabled).toBe(false);
});
it('recovers a backend reservation and unlocks when the operation finishes', async () => {
  vi.useFakeTimers(); state.busy = true; await render(); expect(host.querySelector('select')?.disabled).toBe(true);
  state.busy = false; await act(async () => vi.advanceTimersByTimeAsync(5000));
  expect(host.querySelector('select')?.disabled).toBe(false);
});
it('keeps an administrator cancellation visible after refreshing the actual setting', async () => {
  bridge.setShaderCacheLimit.mockRejectedValueOnce(new Error('Windows administrator approval was cancelled.'));
  await render(); await choose('10'); await act(async () => button('Apply limit').click());
  expect(host.textContent).toContain('Windows administrator approval was cancelled.'); expect(host.querySelector('select')?.value).toBe('100');
});
it('locks size changes until a changed tool connection is saved', async () => {
  await render(true); expect(host.querySelector('select')?.disabled).toBe(true); expect(host.textContent).toContain('Save your SCSKiller connection');
});
it('shows unsupported GPUs and missing tools without exposing a working Apply action', async () => {
  state.configurable = false; await render(); expect(host.textContent).toContain('cannot change the cache limit'); expect(host.querySelector('select')).toBeNull();
  state.installed = false; await act(async () => button('Refresh').click());
  expect(host.textContent).toContain('Connect SCSKiller and save Settings'); expect(bridge.setShaderCacheLimit).not.toHaveBeenCalled();
});
it('blocks applying stale driver data after a query failure', async () => {
  await render(); await choose('10'); bridge.shaderCacheState.mockRejectedValueOnce(new Error('Driver unavailable.'));
  await act(async () => button('Refresh').click()); expect(button('Apply limit').disabled).toBe(true); expect(host.textContent).toContain('Driver unavailable.');
});
