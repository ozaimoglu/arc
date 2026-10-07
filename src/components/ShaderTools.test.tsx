// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { beforeEach, afterEach, it, expect, vi } from 'vitest';
import { demoGames } from '../demo';
import type { ShaderSnapshot } from '../types';
const bridge = vi.hoisted(() => ({ shaderState: vi.fn(), startShaderJob: vi.fn(), stopShaderJob: vi.fn() }));
vi.mock('../bridge', () => ({ isDesktop: true, api: bridge }));
import ShaderTools from './ShaderTools';
let root: Root; let host: HTMLDivElement; let snapshot: ShaderSnapshot;
const configure = vi.fn();
beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
  snapshot = { installed: true, busy: false, job: null, game: null };
  bridge.shaderState.mockImplementation(async () => structuredClone(snapshot));
  bridge.startShaderJob.mockResolvedValue(undefined); bridge.stopShaderJob.mockResolvedValue(undefined);
  host = document.createElement('div'); document.body.append(host); root = createRoot(host);
});
afterEach(async () => { await act(async () => root.unmount()); host.remove(); vi.useRealTimers(); vi.clearAllMocks(); vi.unstubAllGlobals(); });
async function render(game = demoGames[0]) { await act(async () => root.render(<ShaderTools game={game} onConfigure={configure} />)); }
function button(text: string) { return Array.from(host.querySelectorAll('button')).find(button => button.textContent?.includes(text))!; }
it('requires analysis instead of enabling compile for unknown games', async () => {
  await render(); expect(button('Compile shaders').disabled).toBe(true);
  await act(async () => button('Analyze game').click()); expect(bridge.startShaderJob).toHaveBeenCalledWith(1, 'analyze');
});
it('keeps recording-required games disabled and shows the actual reason', async () => {
  snapshot.game = { id: 'steam:1', status: 'NeedsRecording', reason: 'This engine needs a recording.', engine: 'Unreal', graphicsApi: 'D3D12', antiCheat: 'None', shaderCount: null, warmedAt: null, driver: null, canCompile: false };
  await render(); expect(host.textContent).toContain('This engine needs a recording.'); expect(button('Compile shaders').disabled).toBe(true);
});
it('uses the database id and compile action for a verified ready game', async () => {
  snapshot.game = { id: 'steam:1', status: 'Ready', reason: 'Shader files found.', engine: 'Unreal', graphicsApi: 'D3D12', antiCheat: 'None', shaderCount: 120, warmedAt: null, driver: null, canCompile: true };
  await render(); expect(button('Compile shaders').disabled).toBe(false);
  await act(async () => button('Compile shaders').click()); expect(bridge.startShaderJob).toHaveBeenCalledWith(1, 'compile');
});
it('recovers a global job and can stop it after navigating to another game', async () => {
  snapshot.busy = true; snapshot.job = { gameId: 2, title: 'Other game', action: 'compile', running: true, phase: 'Warming', lines: ['512 / 1024'], error: null, stopped: false };
  await render(); expect(host.textContent).toContain('Compiling shaders · Other game'); expect(button('Analyze game').disabled).toBe(true);
  await act(async () => button('Stop').click()); expect(bridge.stopShaderJob).toHaveBeenCalledWith(2);
});
it('opens configuration when the external tool is missing', async () => {
  snapshot.installed = false; await render();
  await act(async () => button('Connect SCSKiller').click()); expect(configure).toHaveBeenCalledOnce(); expect(bridge.startShaderJob).not.toHaveBeenCalled();
});
it('keeps PS4 games out of the native shader workflow', async () => {
  await render({ ...demoGames[0], consoleLaunch: { kind: 'shadPs4', emulator: 'shadPS4.exe', titleId: 'CUSA03173' } });
  expect(host.textContent).toContain('shadPS4 manages its own shader cache'); expect(bridge.shaderState).not.toHaveBeenCalled(); expect(host.querySelector('button')).toBeNull();
});
it('allows a fresh analysis when SCSKiller cached data is corrupt', async () => {
  snapshot.warning = 'SCSKiller data could not be read.';
  await render(); expect(host.textContent).toContain(snapshot.warning); expect(button('Analyze game').disabled).toBe(false);
});
it('retains an operation error when the next background poll succeeds', async () => {
  vi.useFakeTimers(); bridge.startShaderJob.mockRejectedValueOnce(new Error('Close this game first.'));
  await render(); await act(async () => button('Analyze game').click());
  await act(async () => vi.advanceTimersByTimeAsync(5000));
  expect(host.textContent).toContain('Close this game first.');
});
