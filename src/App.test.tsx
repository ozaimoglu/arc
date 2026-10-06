// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { demoGames } from './demo';
import type { Game } from './types';

const bridge = vi.hoisted(() => ({ snapshot: vi.fn(), subscribe: vi.fn(), patch: vi.fn(), launch: vi.fn() }));
vi.mock('./bridge', () => ({ isDesktop: true, imageSrc: (path: string) => path, api: bridge }));
vi.mock('./ratings/useRatings', () => ({ useRatings: () => false }));
import App from './App';

let root: Root;
let host: HTMLDivElement;
let games: Game[];

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
  bridge.snapshot.mockImplementation(async () => structuredClone({ games, settings: { folders: ['D:\\Games'], displayName: 'Player', apiKey: '', autoWatch: true } }));
  bridge.subscribe.mockResolvedValue(() => {});
  bridge.launch.mockResolvedValue(undefined);
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
});
