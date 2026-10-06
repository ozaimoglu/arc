// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, expect, it, vi } from 'vitest';
import { demoGames } from '../demo';
import GameRatings, { RatingSummary } from './GameRatings';

const bridge = vi.hoisted(() => ({ openRatingSource: vi.fn(async () => {}), saveRatings: vi.fn() }));
const client = vi.hoisted(() => ({ loadRatings: vi.fn() }));
vi.mock('../bridge', () => ({ isDesktop: true, api: bridge }));
vi.mock('../ratings/client', async importOriginal => ({ ...await importOriginal<object>(), loadRatings: client.loadRatings }));
const host = document.createElement('div');
document.body.append(host);
const root = createRoot(host);
vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
afterEach(async () => { await act(async () => root.render(null)); vi.clearAllMocks(); });

it('shows separate critic/user scales, Steam review count and source navigation', async () => {
  const game = structuredClone(demoGames[0]);
  game.ratings = { title: game.title, name: game.title, appId: 1245620, status: 'ok', checkedAt: Date.now(), errors: [], metacritic: { score: 96, userScore: 7.8, url: 'https://www.metacritic.com/game/elden-ring/?platform=pc', userUrl: 'https://www.metacritic.com/game/elden-ring/user-reviews/?platform=pc', source: 'steam', platform: 'PC' }, steam: { positive: 940, negative: 60, total: 1000, percent: 94 } };
  await act(async () => root.render(<><RatingSummary game={game} /><GameRatings game={game} /></>));
  expect([...host.querySelectorAll('.rating-number')].map(node => node.textContent)).toEqual(['96/100', '7.8/10', '94%']);
  expect(host.textContent).toContain(`${(1000).toLocaleString()} reviews`);
  expect(host.querySelector('.card-ratings')?.textContent).toBe('MC 96/7.8 · Steam 94');
  expect(host.querySelector('.card-ratings [aria-label^="Steam"]')?.getAttribute('aria-label')).toBe('Steam: 94 percent positive reviews');
  expect([...host.querySelectorAll('.rating-number')].map(node => node.getAttribute('data-tone'))).toEqual(['good', 'good', 'good']);
  await act(async () => host.querySelector<HTMLButtonElement>('.rating-source')!.click());
  expect(bridge.openRatingSource).toHaveBeenCalledWith(game.ratings.metacritic?.url);
});
it('shows missing PS4 ratings as unavailable and offers a working refresh', async () => {
  const game = structuredClone(demoGames[0]);
  game.consoleLaunch = { kind: 'shadPs4', emulator: 'D:\\shadPS4.exe', titleId: 'CUSA03173' };
  client.loadRatings.mockResolvedValue(undefined);
  await act(async () => root.render(<GameRatings game={game} />));
  expect(host.querySelectorAll('.rating-reading')).toHaveLength(2);
  expect(host.textContent).toContain('Critics · PS4'); expect(host.textContent).toContain('—/100');
  expect(host.textContent).not.toContain('Positive reviews');
  expect([...host.querySelectorAll('.rating-number')].every(node => node.getAttribute('data-tone') === 'unavailable')).toBe(true);
  await act(async () => host.querySelector<HTMLButtonElement>('.ratings-heading button')!.click());
  expect(client.loadRatings).toHaveBeenCalledWith(game, true);
});
it('keeps Metacritic users visible when other sources are unavailable, including a genuine zero', async () => {
  const game = structuredClone(demoGames[0]);
  game.ratings = { title: game.title, name: game.title, appId: null, status: 'ok', checkedAt: Date.now(), errors: [], metacritic: { score: null, userScore: 0, url: null, source: 'metacritic', platform: 'PC' }, steam: null };
  await act(async () => root.render(<><RatingSummary game={game} /><GameRatings game={game} /></>));
  expect(host.querySelector('.card-ratings')?.textContent).toBe('MC —/0.0');
  expect(host.querySelector('.card-ratings [aria-label^="Metacritic users"]')?.getAttribute('aria-label')).toBe('Metacritic users: 0.0 out of 10, PC');
  expect([...host.querySelectorAll('.card-ratings b')].map(node => node.getAttribute('data-tone'))).toEqual(['unavailable', 'low']);
  expect([...host.querySelectorAll('.rating-number')].map(node => node.textContent)).toEqual(['—/100', '0.0/10', '—%']);
  expect([...host.querySelectorAll('.rating-number')].map(node => node.getAttribute('data-tone'))).toEqual(['unavailable', 'low', 'unavailable']);
});
it('colors critic, user and Steam readings on their proper scales in both library and details', async () => {
  const game = structuredClone(demoGames[0]);
  game.ratings = { title: game.title, name: game.title, appId: 1245620, status: 'ok', checkedAt: Date.now(), errors: [], metacritic: { score: 75, userScore: 5, url: null, source: 'metacritic', platform: 'PC' }, steam: { positive: 49, negative: 51, total: 100, percent: 49 } };
  await act(async () => root.render(<><RatingSummary game={game} /><GameRatings game={game} /></>));
  expect([...host.querySelectorAll('.card-ratings b')].map(node => node.getAttribute('data-tone'))).toEqual(['good', 'mixed', 'low']);
  expect([...host.querySelectorAll('.rating-number')].map(node => node.getAttribute('data-tone'))).toEqual(['good', 'mixed', 'low']);
  expect(host.querySelector('.card-ratings')?.textContent).toBe('MC 75/5.0 · Steam 49');
});
