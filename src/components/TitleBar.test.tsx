// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DesktopWindow } from '../bridge';

const bridge = vi.hoisted(() => ({ getDesktopWindow: vi.fn() }));
vi.mock('../bridge', () => bridge);
import TitleBar from './TitleBar';
import Modal from './Modal';

const appWindow = {
  minimize: vi.fn(), toggleMaximize: vi.fn(), close: vi.fn(),
  isMaximized: vi.fn(), isFocused: vi.fn(), onResized: vi.fn(), onFocusChanged: vi.fn(),
};
let host: HTMLDivElement;
let root: Root;
let mounted: boolean;
let resized: () => void;
let focusChanged: (event: { payload: boolean }) => void;
let offResize: ReturnType<typeof vi.fn>;
let offFocus: ReturnType<typeof vi.fn>;
let onError: ReturnType<typeof vi.fn>;

beforeEach(() => {
  vi.resetAllMocks();
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
  for (const name of ['minimize', 'toggleMaximize', 'close'] as const) appWindow[name].mockResolvedValue(undefined);
  appWindow.isMaximized.mockResolvedValue(false);
  appWindow.isFocused.mockResolvedValue(true);
  offResize = vi.fn(); offFocus = vi.fn(); onError = vi.fn();
  appWindow.onResized.mockImplementation(async (handler: () => void) => { resized = handler; return offResize; });
  appWindow.onFocusChanged.mockImplementation(async (handler: (event: { payload: boolean }) => void) => { focusChanged = handler; return offFocus; });
  bridge.getDesktopWindow.mockReturnValue(appWindow satisfies DesktopWindow);
  host = document.createElement('div'); document.body.append(host);
  root = createRoot(host); mounted = true;
});
afterEach(async () => {
  if (mounted) await act(async () => root.unmount());
  host.remove(); vi.unstubAllGlobals();
});

async function click(label: string) {
  const button = host.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`);
  expect(button).not.toBeNull();
  await act(async () => button!.click());
}

describe('desktop title bar', () => {
  it('routes accessible controls to the native window and reports a failed action', async () => {
    await act(async () => root.render(<TitleBar onError={onError} />));
    await click('Minimize window'); await click('Maximize window'); await click('Close Arc');
    expect(appWindow.minimize).toHaveBeenCalledOnce();
    expect(appWindow.toggleMaximize).toHaveBeenCalledOnce();
    expect(appWindow.close).toHaveBeenCalledOnce();
    appWindow.minimize.mockRejectedValueOnce(new Error('Native operation failed'));
    await click('Minimize window');
    expect(onError).toHaveBeenCalledWith('Window controls: Error: Native operation failed');
  });

  it('keeps the latest native maximize state when resize queries finish out of order', async () => {
    await act(async () => root.render(<TitleBar onError={onError} />));
    let finishOld!: (value: boolean) => void;
    appWindow.isMaximized.mockImplementationOnce(() => new Promise<boolean>(resolve => { finishOld = resolve; })).mockResolvedValueOnce(true);
    await act(async () => { resized(); resized(); focusChanged({ payload: false }); });
    expect(host.querySelector('[aria-label="Restore window"]')).not.toBeNull();
    expect(host.querySelector('.window-titlebar.inactive')).not.toBeNull();
    await act(async () => finishOld(false));
    expect(host.querySelector('[aria-label="Restore window"]')).not.toBeNull();
    await click('Restore window');
    expect(appWindow.toggleMaximize).toHaveBeenCalledOnce();
  });

  it('removes event subscriptions even if registration completes after unmount', async () => {
    let finishRegistration!: (value: () => void) => void;
    appWindow.onResized.mockReturnValueOnce(new Promise<() => void>(resolve => { finishRegistration = resolve; }));
    await act(async () => root.render(<TitleBar onError={onError} />));
    await act(async () => root.unmount()); mounted = false;
    expect(offFocus).toHaveBeenCalledOnce();
    expect(offResize).not.toHaveBeenCalled();
    await act(async () => finishRegistration(offResize));
    expect(offResize).toHaveBeenCalledOnce();
  });

  it('keeps native controls inside the modal top layer without dismissing the dialog', async () => {
    const onClose = vi.fn();
    await act(async () => root.render(<Modal title="Settings" onClose={onClose}><p>Settings content</p></Modal>));
    const dialog = host.querySelector('dialog')!;
    expect(dialog.open).toBe(true);
    expect(dialog.querySelector('[aria-label="Window controls"]')).not.toBeNull();
    await click('Minimize window');
    expect(appWindow.minimize).toHaveBeenCalledOnce();
    expect(onClose).not.toHaveBeenCalled();
    await act(async () => dialog.querySelector<HTMLElement>('.modal')!.click());
    expect(onClose).not.toHaveBeenCalled();
    await act(async () => dialog.click());
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('omits desktop window controls in the browser preview', async () => {
    bridge.getDesktopWindow.mockReturnValue(null);
    await act(async () => root.render(<TitleBar onError={onError} />));
    expect(host.querySelector('[aria-label="Window controls"]')).toBeNull();
    expect(appWindow.onResized).not.toHaveBeenCalled();
  });
});
