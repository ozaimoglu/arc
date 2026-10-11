// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { demoGames } from '../demo';
import type { Game, GameRatings } from '../types';

const client = vi.hoisted(() => ({ loadRatings: vi.fn() }));
vi.mock('../bridge', () => ({ isDesktop: true, api: { ratingsCacheRead: vi.fn(), ratingsCacheWrite: vi.fn() } }));
vi.mock('./client', async original => ({ ...await original<object>(), loadRatings: client.loadRatings }));
import { useRatings } from './useRatings';

const HOURS_72 = 72 * 3600000;
let host: HTMLDivElement;
let root: Root;
let mounted: boolean;
let games: Game[];
let priority: number | null;

function saved(game: Game, checkedAt = Date.now()): GameRatings {
  return { title: game.title, name: game.title, appId: 123, status: 'ok', checkedAt, errors: [], metacritic: null, steam: { positive: 9, negative: 1, total: 10, percent: 90 } };
}
function Harness() {
  const busy = useRatings(games, priority);
  return <span>{busy ? 'Updating' : 'Idle'}</span>;
}
async function render() { await act(async () => root.render(<Harness />)); }
async function advance(milliseconds: number) { await act(async () => vi.advanceTimersByTimeAsync(milliseconds)); }

beforeEach(() => {
  vi.useFakeTimers(); vi.setSystemTime(0);
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
  client.loadRatings.mockReset();
  games = [structuredClone(demoGames[0])]; games[0].ratings = saved(games[0], 0);
  priority = null;
  client.loadRatings.mockImplementation(async (game: Game) => {
    const value = saved(game); game.ratings = value; return value;
  });
  host = document.createElement('div'); document.body.append(host); root = createRoot(host); mounted = true;
});
afterEach(async () => {
  if (mounted) await act(async () => root.unmount());
  host.remove(); vi.useRealTimers(); vi.unstubAllGlobals();
});

describe('automatic rating refresh', () => {
  it('refreshes at exactly 72 hours and repeats while the app stays mounted', async () => {
    await render();
    await advance(HOURS_72 - 1); expect(client.loadRatings).not.toHaveBeenCalled();
    await advance(1); expect(client.loadRatings).toHaveBeenCalledTimes(1);
    await advance(HOURS_72 - 1); expect(client.loadRatings).toHaveBeenCalledTimes(1);
    await advance(1); expect(client.loadRatings).toHaveBeenCalledTimes(2);
    expect(host.textContent).toBe('Idle');
  });

  it('checks expired ratings after resuming even when the original timer did not fire', async () => {
    await render(); vi.setSystemTime(HOURS_72 + 1);
    await act(async () => window.dispatchEvent(new Event('focus')));
    expect(client.loadRatings).toHaveBeenCalledTimes(1);
    await act(async () => window.dispatchEvent(new Event('online')));
    expect(client.loadRatings).toHaveBeenCalledTimes(1);
  });

  it('prioritizes details, excludes hidden games and keeps one request running', async () => {
    games = [1, 2, 3].map(id => ({ ...structuredClone(demoGames[0]), id, hidden: id === 3, ratings: null }));
    priority = 2;
    let finish!: (value: GameRatings) => void;
    client.loadRatings.mockImplementationOnce(() => new Promise<GameRatings>(resolve => { finish = resolve; }));
    await render(); expect(client.loadRatings.mock.calls.map(([game]) => game.id)).toEqual([2]);
    await act(async () => { window.dispatchEvent(new Event('focus')); window.dispatchEvent(new Event('online')); });
    expect(client.loadRatings).toHaveBeenCalledTimes(1);
    await act(async () => finish(saved(games[1])));
    expect(client.loadRatings.mock.calls.map(([game]) => game.id)).toEqual([2, 1]);
  });

  it('keeps cached scores and backs off failed refreshes without a retry loop', async () => {
    vi.setSystemTime(HOURS_72);
    const previous = structuredClone(games[0].ratings);
    client.loadRatings.mockRejectedValueOnce(new Error('Offline'));
    await render(); expect(client.loadRatings).toHaveBeenCalledTimes(1);
    expect(games[0].ratings).toEqual(previous);
    await advance(15 * 60000 - 1); expect(client.loadRatings).toHaveBeenCalledTimes(1);
    await advance(1); expect(client.loadRatings).toHaveBeenCalledTimes(2);
    await advance(60000); expect(client.loadRatings).toHaveBeenCalledTimes(2);
  });

  it('removes its timer and resume listeners when the app unmounts', async () => {
    await render(); expect(vi.getTimerCount()).toBe(1);
    await act(async () => root.unmount()); mounted = false;
    expect(vi.getTimerCount()).toBe(0);
    vi.setSystemTime(HOURS_72 + 1);
    window.dispatchEvent(new Event('focus')); window.dispatchEvent(new Event('online'));
    expect(client.loadRatings).not.toHaveBeenCalled();
  });

  it('starts a new 72-hour period after a manual refresh changes the saved timestamp', async () => {
    await render(); await advance(24 * 3600000);
    games[0].ratings = saved(games[0]); await render();
    await advance(48 * 3600000); expect(client.loadRatings).not.toHaveBeenCalled();
    await advance(24 * 3600000); expect(client.loadRatings).toHaveBeenCalledOnce();
  });
});
