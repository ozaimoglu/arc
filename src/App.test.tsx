// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { demoGames } from './demo';
import type { Game, Snapshot } from './types';

const bridge = vi.hoisted(() => ({ snapshot: vi.fn(), subscribe: vi.fn(), patch: vi.fn(), launch: vi.fn(), shaderState: vi.fn() }));
vi.mock('./bridge', () => ({ isDesktop: true, imageSrc: (path: string) => path, api: bridge }));
vi.mock('./ratings/useRatings', () => ({ useRatings: () => false }));
import App from './App';

let root: Root;
let host: HTMLDivElement;
let games: Game[];
let runningGameIds: number[];
let libraryChanged: () => void;

function snapshot(): Snapshot {
  return structuredClone({ games, runningGameIds, settings: { folders: ['D:\\Games'], displayName: 'Player', apiKey: '', autoWatch: true } });
}

async function click(selector: string) {
  const button = host.querySelector<HTMLButtonElement>(selector);
  expect(button, selector).not.toBeNull();
  await act(async () => button!.click());
}

beforeEach(async () => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
  vi.stubGlobal('matchMedia', () => ({ matches: true }));
  vi.spyOn(HTMLElement.prototype, 'scrollTo').mockImplementation(function (this: HTMLElement, options: number | ScrollToOptions, y?: number) {
    this.scrollTop = typeof options === 'object' ? options.top ?? this.scrollTop : y ?? this.scrollTop;
  });
  games = structuredClone(demoGames);
  games[7].hidden = true;
  runningGameIds = [];
  bridge.snapshot.mockImplementation(async () => snapshot());
  bridge.subscribe.mockImplementation(async (handler: () => void) => { libraryChanged = handler; return () => {}; });
  bridge.launch.mockImplementation(async (id: number) => { runningGameIds.push(id); });
  bridge.shaderState.mockResolvedValue({ installed: false, game: null, job: null, busy: false });
  bridge.patch.mockImplementation(async (id: number, patch: Partial<Game>) => {
    Object.assign(games.find(game => game.id === id)!, patch);
  });
  host = document.createElement('div');
  document.body.append(host);
  root = createRoot(host);
  await act(async () => root.render(<App />));
});

afterEach(async () => {
  await act(async () => root.unmount());
  host.remove();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe('cinematic library interactions', () => {
  it('changes the featured game explicitly without changing game records', async () => {
    const before = structuredClone(games);
    const choices = host.querySelectorAll<HTMLButtonElement>('.feature-thumbnail');
    const title = choices[1].getAttribute('title');
    await act(async () => choices[1].click());
    expect(host.querySelector('.feature-hero')?.getAttribute('aria-label')).toBe(`Featured game: ${title}`);
    expect(choices[1].getAttribute('aria-pressed')).toBe('true');
    expect(games).toEqual(before);
  });
  it('opens a game, restores library scroll and returns focus to its cover', async () => {
    const main = host.querySelector<HTMLElement>('main')!;
    main.scrollTop = 700;
    await click('[data-game-id="1"]');
    expect(host.querySelector('.detail-page')).not.toBeNull();
    expect(main.scrollTop).toBe(0);
    expect(document.activeElement).toBe(host.querySelector('.back-button'));
    await click('.back-button');
    expect(host.querySelector('.detail-page')).toBeNull();
    expect(main.scrollTop).toBe(700);
    expect(document.activeElement).toBe(host.querySelector('[data-game-id="1"]'));
  });
  it('searches from the header, hides the hero and clears back to the library', async () => {
    const input = host.querySelector<HTMLInputElement>('#library-search')!;
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!;
    await act(async () => { setter.call(input, 'ELDEN'); input.dispatchEvent(new Event('input', { bubbles: true })); });
    expect(host.querySelector('.feature-hero')).toBeNull();
    expect(host.querySelectorAll('.game-card')).toHaveLength(1);
    expect(host.querySelector('.card-title')?.textContent).toBe('ELDEN RING');
    await click('[aria-label="Clear search"]');
    expect(host.querySelectorAll('.game-card')).toHaveLength(games.filter(game => !game.hidden).length);
    expect(host.querySelector('.feature-hero')).not.toBeNull();
  });
  it('keeps grid/list navigation and hidden-game restoration working', async () => {
    await click('[aria-label="List view"]');
    expect(host.querySelector('.list-layout')).not.toBeNull();
    await click('[aria-label="Hidden games"]');
    expect(host.querySelectorAll('.game-card')).toHaveLength(1);
    await click('.restore-button');
    expect(bridge.patch).toHaveBeenCalledWith(8, { hidden: false });
    expect(host.querySelectorAll('.game-card')).toHaveLength(0);
    await click('.main-nav button:first-child');
    expect(host.querySelectorAll('.game-card')).toHaveLength(games.length);
  });
  it('offers keyboard-accessible game menus and sends Play to the desktop bridge', async () => {
    await click('[aria-label="Actions for ELDEN RING"]');
    expect(host.querySelector('[role="menu"]')).not.toBeNull();
    expect(document.activeElement).toBe(host.querySelector('[role="menuitem"]'));
    await click('[role="menuitem"]');
    expect(bridge.launch).toHaveBeenCalledWith(1);
    expect(host.querySelector('[role="menu"]')).toBeNull();
    expect(host.querySelector('[role="status"]')?.textContent).toContain('Starting ELDEN RING');
  });
  it('disables every Play entry while launching and running, then enables them when the process exits', async () => {
    let started!: () => void;
    bridge.launch.mockImplementationOnce(() => new Promise<void>(resolve => { started = () => { runningGameIds = [1]; resolve(); }; }));
    await click('[data-game-id="1"]');
    await click('.play-button');
    expect(host.querySelector<HTMLButtonElement>('.play-button')?.disabled).toBe(true);
    expect(host.querySelector('.play-button')?.textContent).toBe('Launching…');
    await click('[aria-label="Actions for ELDEN RING"]');
    const menuPlay = host.querySelector<HTMLButtonElement>('[role="menuitem"]')!;
    expect(menuPlay.disabled).toBe(true);
    expect(menuPlay.textContent).toBe('Launching…');
    expect(document.activeElement).toBe(menuPlay.nextElementSibling);
    await act(async () => {
      menuPlay.click();
      document.activeElement!.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true }));
    });
    expect(document.activeElement?.textContent).toBe('Remove from library');
    await act(async () => document.activeElement!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })));
    await act(async () => started());
    expect(host.querySelector<HTMLButtonElement>('.play-button')?.disabled).toBe(true);
    expect(host.querySelector('.play-button')?.textContent).toBe('Running');
    await click('.play-button');
    await click('[aria-label="Actions for ELDEN RING"]');
    expect(host.querySelector<HTMLButtonElement>('[role="menuitem"]')?.disabled).toBe(true);
    expect(host.querySelector('[role="menuitem"]')?.textContent).toBe('Running');
    await click('[role="menuitem"]');
    expect(bridge.launch).toHaveBeenCalledTimes(1);
    await act(async () => document.activeElement!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })));
    await click('.back-button');
    expect(host.querySelector<HTMLButtonElement>('.play-button')?.disabled).toBe(true);
    expect(host.querySelector<HTMLButtonElement>('[aria-label="Running ELDEN RING"]')?.disabled).toBe(true);
    expect(host.querySelector<HTMLButtonElement>('[aria-label="Play Cyberpunk 2077"]')?.disabled).toBe(false);
    await click('[aria-label="List view"]');
    expect([...host.querySelectorAll<HTMLButtonElement>('[aria-label="Running ELDEN RING"]')].every(button => button.disabled)).toBe(true);
    runningGameIds = [];
    await act(async () => libraryChanged());
    expect(host.querySelector<HTMLButtonElement>('.play-button')?.disabled).toBe(false);
    const quickPlay = host.querySelectorAll<HTMLButtonElement>('[aria-label="Play ELDEN RING"]');
    expect(quickPlay).toHaveLength(2);
    expect([...quickPlay].every(button => !button.disabled)).toBe(true);
    await click('[aria-label="Actions for ELDEN RING"]');
    expect(host.querySelector<HTMLButtonElement>('[role="menuitem"]')?.disabled).toBe(false);
  });
  it('restores Play after a launch error and allows retrying', async () => {
    bridge.launch.mockImplementationOnce(async () => {
      runningGameIds = [1];
      libraryChanged();
      await Promise.resolve();
      runningGameIds = [];
      throw new Error('Windows could not start this game.');
    });
    await click('[data-game-id="1"]');
    await click('.play-button');
    expect(host.querySelector<HTMLButtonElement>('.play-button')?.disabled).toBe(false);
    expect(host.querySelector('.play-button')?.textContent).toBe('Play');
    expect(host.querySelector('[role="alert"]')?.textContent).toContain('Windows could not start');
    await click('.play-button');
    expect(bridge.launch).toHaveBeenCalledTimes(2);
    expect(host.querySelector('.play-button')?.textContent).toBe('Running');
  });
  it('blocks a second launch before React commits the disabled state', async () => {
    let started!: () => void;
    bridge.launch.mockImplementationOnce(() => new Promise<void>(resolve => { started = () => { runningGameIds = [1]; resolve(); }; }));
    await click('[data-game-id="1"]');
    const play = host.querySelector<HTMLButtonElement>('.play-button')!;
    await act(async () => { play.click(); play.click(); });
    expect(bridge.launch).toHaveBeenCalledTimes(1);
    expect(play.disabled).toBe(true);
    await act(async () => started());
    expect(play.textContent).toBe('Running');
  });
  it('loads running status when the interface is reopened', async () => {
    await act(async () => root.render(null));
    runningGameIds = [1];
    await act(async () => root.render(<App />));
    expect(host.querySelector<HTMLButtonElement>('[aria-label="Running ELDEN RING"]')?.disabled).toBe(true);
    expect(host.querySelector<HTMLButtonElement>('[aria-label="Play Cyberpunk 2077"]')?.disabled).toBe(false);
    expect(bridge.launch).not.toHaveBeenCalled();
  });
  it('ignores delayed snapshots that would restore outdated process status', async () => {
    runningGameIds = [1];
    const stale = snapshot();
    let complete!: (value: Snapshot) => void;
    bridge.snapshot.mockImplementationOnce(() => new Promise<Snapshot>(resolve => { complete = resolve; }));
    await act(async () => libraryChanged());
    runningGameIds = [];
    await act(async () => libraryChanged());
    await act(async () => complete(stale));
    expect(host.querySelector<HTMLButtonElement>('[aria-label="Play ELDEN RING"]')?.disabled).toBe(false);
    expect(host.querySelector('[aria-label="Running ELDEN RING"]')).toBeNull();
  });
  it('keeps a successfully launched game disabled if its library refresh fails', async () => {
    bridge.snapshot.mockRejectedValueOnce(new Error('Could not refresh the library.'));
    await click('[data-game-id="1"]');
    await click('.play-button');
    expect(host.querySelector<HTMLButtonElement>('.play-button')?.disabled).toBe(true);
    expect(host.querySelector('.play-button')?.textContent).toBe('Running');
    runningGameIds = [];
    await act(async () => libraryChanged());
    expect(host.querySelector<HTMLButtonElement>('.play-button')?.disabled).toBe(false);
  });
});
