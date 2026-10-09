// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { beforeEach, afterEach, it, expect, vi } from 'vitest';
import { demoGames } from '../demo';
import type { ShaderSnapshot } from '../types';
const bridge = vi.hoisted(() => ({ shaderState: vi.fn(), startShaderJob: vi.fn(), stopShaderJob: vi.fn(), clearShaderCache: vi.fn() }));
vi.mock('../bridge', () => ({ isDesktop: true, getDesktopWindow: () => null, api: bridge }));
import ShaderTools from './ShaderTools';
let root: Root; let host: HTMLDivElement; let snapshot: ShaderSnapshot;
const configure = vi.fn();
beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
  snapshot = { installed: true, busy: false, job: null, game: null };
  bridge.shaderState.mockImplementation(async () => structuredClone(snapshot));
  bridge.startShaderJob.mockResolvedValue(undefined); bridge.stopShaderJob.mockResolvedValue(undefined); bridge.clearShaderCache.mockResolvedValue(undefined);
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
it('distinguishes an unconfirmed recording folder from unsupported shader formats', async () => {
  snapshot.game = { id: 'manual:abc', status: 'Unsupported', reason: 'needs a recording, once you confirm its game folder', engine: 'Carved', graphicsApi: 'D3D12', antiCheat: 'None', shaderCount: null, warmedAt: null, driver: null, canCompile: false };
  await render(); expect(host.textContent).toContain('Folder confirmation needed');
  expect(host.textContent).toContain('Confirm this game’s folder in SCSKiller'); expect(host.textContent).not.toContain('Not supported');
  expect(button('Compile shaders').disabled).toBe(true);
});
it('uses the database id and compile action for a verified ready game', async () => {
  snapshot.game = { id: 'steam:1', status: 'Ready', reason: 'Shader files found.', engine: 'Unreal', graphicsApi: 'D3D12', antiCheat: 'None', shaderCount: 120, warmedAt: null, driver: null, canCompile: true };
  await render(); expect(button('Compile shaders').disabled).toBe(false);
  await act(async () => button('Compile shaders').click()); expect(bridge.startShaderJob).toHaveBeenCalledWith(1, 'compile');
});
it.each(['PartlyCompiled', 'PartlyWarmed', 'CompileUnreached'])('does not show %s as fully prepared', async status => {
  snapshot.game = { id: 'steam:1', status, reason: 'Driver verification result.', engine: 'Unreal', graphicsApi: 'D3D12', antiCheat: 'None', shaderCount: 120, warmedAt: '2026-10-08T12:00:00Z', driver: 'test', canCompile: true };
  await render(); expect(host.textContent).not.toContain('Shaders prepared');
  expect(host.querySelector('.shader-status.prepared')).toBeNull(); expect(host.textContent).toContain('Driver verification result.');
});
it('shows the no-stutter verdict without asking for an unnecessary recording', async () => {
  snapshot.game = { id: 'steam:1', status: 'NoShaderStutter', reason: 'The game precompiles its shaders.', engine: 'Unreal', graphicsApi: 'D3D12', antiCheat: 'None', shaderCount: null, warmedAt: null, driver: null, canCompile: false, canRecord: false, recorderInstalled: true };
  await render(); expect(host.textContent).toContain('No shader stutter'); expect(host.textContent).not.toContain('five minutes');
  expect(button('Compile shaders').disabled).toBe(true); expect(button('Prepare recording')).toBeUndefined();
});
it('directs offline recordings to the external tool without offering recorder setup', async () => {
  snapshot.recordingSupported = true;
  snapshot.game = { id: 'steam:1', status: 'NeedsOfflineSession', reason: 'EasyAntiCheat blocks recording.', engine: 'Unreal', graphicsApi: 'D3D12', antiCheat: 'EasyAntiCheat', shaderCount: null, warmedAt: null, driver: null, canCompile: false, canRecord: false };
  await render(); expect(host.textContent).toContain('Offline session needed'); expect(host.textContent).toContain('Manage this game’s offline recording in SCSKiller');
  expect(button('Prepare recording')).toBeUndefined(); expect(button('Compile shaders').disabled).toBe(true);
});
function needsRecorder() {
  snapshot.recordingSupported = true;
  snapshot.game = { id: 'manual:abc', status: 'Unsupported', reason: 'needs a recording, once you confirm its game folder', engine: 'Carved', graphicsApi: 'D3D12', antiCheat: 'None', shaderCount: null, warmedAt: null, driver: null, canCompile: false, canRecord: true, recorderInstalled: false };
}
it('offers recording setup with explicit confirmation and the exact database identity', async () => {
  needsRecorder(); await render();
  await act(async () => button('Prepare recording').click());
  expect(bridge.startShaderJob).not.toHaveBeenCalled();
  expect(host.querySelector('dialog')?.textContent).toContain(demoGames[0].title);
  expect(host.querySelector('dialog')?.textContent).toContain('recorder DLL');
  await act(async () => button('Install recorder').click());
  expect(bridge.startShaderJob).toHaveBeenCalledWith(1, 'prepareRecording');
});
it('keeps recorder setup unavailable for incompatible engines and older tools', async () => {
  needsRecorder(); snapshot.game!.canRecord = false; await render(); expect(button('Prepare recording')).toBeUndefined();
  snapshot.game!.canRecord = true; snapshot.recordingSupported = false; await render(); expect(button('Prepare recording')).toBeUndefined();
});
it('shows captured bytes and gameplay instructions without claiming a compile', async () => {
  needsRecorder(); snapshot.game!.recorderInstalled = true; snapshot.game!.recordingBytes = 2097152;
  await render(); expect(host.textContent).toContain('Recorder ready'); expect(host.textContent).toContain('2.0 MB recorded');
  expect(host.textContent).toContain('five minutes'); expect(button('Compile shaders').disabled).toBe(true); expect(button('Prepare recording')).toBeUndefined();
});
it('preserves recorder setup across navigation without an unsafe Stop action', async () => {
  snapshot.busy = true; snapshot.job = { gameId: 2, title: 'Other game', action: 'prepareRecording', running: true, phase: 'PreparingRecording', lines: [], error: null, stopped: false };
  await render(); expect(host.textContent).toContain('Installing gameplay recorder · Other game'); expect(button('Stop')).toBeUndefined();
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
function readyForCleanup() {
  snapshot.game = { id: 'steam:999', status: 'Unsupported', reason: 'Packed shader format.', engine: null, graphicsApi: null, antiCheat: 'None', shaderCount: null, warmedAt: null, driver: null, canCompile: false };
}
it('requires cleanup confirmation and defaults to preserving game precache', async () => {
  readyForCleanup(); await render();
  await act(async () => button('Clear cache').click());
  expect(bridge.clearShaderCache).not.toHaveBeenCalled();
  expect(host.querySelector('dialog')?.textContent).toContain(demoGames[0].title);
  expect(host.querySelector<HTMLInputElement>('input[type="checkbox"]')?.checked).toBe(false);
  await act(async () => button('Clear shader cache').click());
  expect(bridge.clearShaderCache).toHaveBeenCalledWith(1, false);
  expect(bridge.startShaderJob).not.toHaveBeenCalled();
});
it('can opt into generated game precache deletion using the Arc database identity', async () => {
  readyForCleanup(); await render();
  await act(async () => button('Clear cache').click());
  await act(async () => host.querySelector<HTMLInputElement>('input[type="checkbox"]')!.click());
  await act(async () => button('Clear shader cache').click());
  expect(bridge.clearShaderCache).toHaveBeenCalledWith(1, true);
});
it('cancels cleanup without touching cache files and blocks unverified games', async () => {
  vi.useFakeTimers();
  await render(); expect(button('Clear cache').disabled).toBe(true);
  readyForCleanup(); await act(async () => vi.advanceTimersByTimeAsync(5000));
  await act(async () => button('Clear cache').click());
  await act(async () => button('Keep cache').click());
  expect(host.querySelector('dialog')).toBeNull(); expect(bridge.clearShaderCache).not.toHaveBeenCalled();
});
it('recovers cleanup progress across navigation without offering an unsafe Stop', async () => {
  snapshot.busy = true; snapshot.job = { gameId: 2, title: 'Other game', action: 'clearCache', running: true, phase: 'Clearing', lines: [], error: null, stopped: false };
  await render(); expect(host.textContent).toContain('Clearing shader cache · Other game');
  expect(button('Stop')).toBeUndefined(); expect(button('Clear cache').disabled).toBe(true);
});
it('shows verified cleanup completion and the refreshed preparation state', async () => {
  readyForCleanup(); snapshot.job = { gameId: 1, title: demoGames[0].title, action: 'clearCache', running: false, phase: 'CacheCleared', lines: ['cleared Game'], error: null, stopped: false };
  await render(); expect(host.textContent).toContain('Shader cache cleared'); expect(button('Analyze again').disabled).toBe(false);
});
