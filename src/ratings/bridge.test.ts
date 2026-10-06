import { beforeEach, expect, it, vi } from 'vitest';
import { demoGames } from '../demo';
import userHtml from './fixtures/metacritic-users.html?raw';

const native = vi.hoisted(() => ({ ratingsCacheRead: vi.fn(), ratingsCacheWrite: vi.fn(), ratingsRequest: vi.fn(), ratingCatalogueTitle: vi.fn(), ratingAppId: vi.fn(), saveRatings: vi.fn() }));
vi.mock('../bridge', () => ({ api: native }));
beforeEach(() => {
  vi.resetModules(); vi.clearAllMocks();
  const cache: Record<string, unknown> = {};
  native.ratingsCacheRead.mockImplementation(async (key: string) => ({ [key]: cache[key] }));
  native.ratingsCacheWrite.mockImplementation(async (values: Record<string, unknown>) => { Object.assign(cache, values); });
  native.saveRatings.mockResolvedValue(undefined);
  native.ratingAppId.mockResolvedValue(null);
  native.ratingsRequest.mockImplementation(async (url: string) => {
    let body: string;
    if (url.includes('storesearch')) body = JSON.stringify({ items: [{ id: 1001, name: 'Arc Ratings Regression', type: 'app' }] });
    else if (url.includes('appdetails')) {
      const id = Number(new URL(url).searchParams.get('appids'));
      body = JSON.stringify({ [id]: { success: true, data: { name: 'Arc Ratings Regression', steam_appid: id, type: 'game', metacritic: { score: 85, url: 'https://www.metacritic.com/game/arc-ratings-regression/' } } } });
    } else if (url.includes('appreviews')) body = JSON.stringify({ success: 1, query_summary: { total_positive: 9, total_negative: 1 } });
    else body = userHtml.replaceAll('FINAL FANTASY TACTICS - The Ivalice Chronicles', 'Arc Ratings Regression').replaceAll('final-fantasy-tactics-the-ivalice-chronicles', 'arc-ratings-regression');
    return { status: 200, body, url, retryAfter: null };
  });
});
it('uses native requests/storage, persists the actual game ID and reuses cache without another fetch', async () => {
  const { loadRatings } = await import('./client');
  const game = { ...structuredClone(demoGames[0]), id: 321, title: 'Arc Ratings Regression' };
  await loadRatings(game);
  const calls = native.ratingsRequest.mock.calls.length;
  expect(native.saveRatings.mock.calls[0].slice(0, 3)).toEqual([321, game.title, 'pc']);
  expect(native.saveRatings.mock.calls[0][3].steam.percent).toBe(90);
  expect(native.saveRatings.mock.calls[0][3].metacritic.userScore).toBe(6.5);
  expect(native.ratingsCacheWrite).toHaveBeenCalled();
  await loadRatings(game);
  expect(native.ratingsRequest.mock.calls.length).toBe(calls);
});
it('publishes a manual Steam correction after an active automatic lookup and reuses its verified ID', async () => {
  const { loadRatings } = await import('./client');
  const game = { ...structuredClone(demoGames[0]), id: 321, title: 'Arc Ratings Regression' };
  await Promise.all([loadRatings(game), loadRatings(game, true, 1002)]);
  expect(native.saveRatings.mock.calls.map(call => call[3].appId)).toEqual([1001, 1002]);
  game.ratings = native.saveRatings.mock.calls[1][3];
  const count = native.ratingsRequest.mock.calls.length;
  await loadRatings(game, true);
  expect(native.ratingsRequest.mock.calls.slice(count).every(([url]) => !url.includes('storesearch'))).toBe(true);
  expect(native.saveRatings.mock.calls[2][3].appId).toBe(1002);
});
it('does not publish reviews for a supplied ID when Steam cannot verify its game identity', async () => {
  const original = native.ratingsRequest.getMockImplementation()!;
  native.ratingsRequest.mockImplementation(async (url: string) => url.includes('appdetails') ? { status: 503, body: '', url, retryAfter: null } : original(url));
  const { loadRatings } = await import('./client');
  const game = { ...structuredClone(demoGames[0]), id: 321, title: 'Arc Ratings Regression' };
  await loadRatings(game, true, 1002);
  expect(native.saveRatings.mock.calls[0][3].status).toBe('unverified');
  expect(native.saveRatings.mock.calls[0][3].steam).toBeNull();
  expect(native.saveRatings.mock.calls[0][3].metacritic).toBeNull();
});
