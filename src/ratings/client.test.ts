import { describe, expect, it, vi } from 'vitest';
vi.mock('../bridge', () => ({ api: { ratingsCacheRead: vi.fn(), ratingsCacheWrite: vi.fn() } }));
import criticHtml from './fixtures/metacritic.html?raw';
import userHtml from './fixtures/metacritic-users.html?raw';
import { hasRatings, ratingsDue, retainRatings } from './client';
import type { GameRatings } from '../types';

const title = 'FINAL FANTASY TACTICS - The Ivalice Chronicles';
const metaTitle = 'final-fantasy-tactics-the-ivalice-chronicles';
function memory() {
  const data: Record<string, unknown> = {};
  return { data, async get(key: string) { return { [key]: data[key] }; }, async set(values: Record<string, unknown>) { Object.assign(data, values); } };
}
function saved(): GameRatings {
  return { title, name: title, appId: 100, status: 'ok', checkedAt: 1000, errors: [], metacritic: { score: 87, userScore: 6.5, url: `https://www.metacritic.com/game/${metaTitle}/?platform=pc`, source: 'metacritic', platform: 'PC' }, steam: { positive: 900, negative: 100, total: 1000, percent: 90 } };
}

describe('CriticPeek identity and platform parsers reused by Arc', () => {
  it('rejects a sequel, DLC, remake mismatch and ambiguous exact matches', () => {
    const app = (id: number, name: string) => ({ id, name, type: 'app' });
    expect(CriticScores.chooseMatch('Hades II', [app(1, 'Hades')])).toBeNull();
    expect(CriticScores.chooseMatch('Cyberpunk 2077', [app(1, 'Cyberpunk 2077: Phantom Liberty')])).toBeNull();
    expect(CriticScores.chooseMatch('Gothic 1 Remake', [app(1, 'Gothic')])).toBeNull();
    expect(CriticScores.chooseMatch('Broken Arrow', [app(1, 'Broken Arrow'), app(2, 'Broken Arrow')])).toBeNull();
    expect(CriticScores.chooseMatch('Hades II', [app(1, 'Hades 2')])?.id).toBe(1);
  });
  it('preserves release identity while normalizing editions and scanner labels', () => {
    expect(CriticScores.cleanTitle('Cyberpunk 2077 [v 1.12] RePack')).toBe('Cyberpunk 2077');
    expect(CriticScores.chooseMatch('Game Deluxe Edition', [{ id: 1, name: 'Game', type: 'app' }])?.id).toBe(1);
    expect(CriticScores.chooseMatch('The Binding of Isaac Rebirth Complete Edition', [{ id: 1, name: 'The Binding of Isaac: Rebirth', type: 'app' }])?.id).toBe(1);
    expect(CriticScores.chooseMatch('Sekiro: Shadows Die Twice', [{ id: 1, name: 'Sekiro: Shadows Die Twice - GOTY Edition', type: 'app' }])?.id).toBe(1);
    expect(CriticScores.chooseMatch('Game', [{ id: 1, name: 'Game Deluxe Edition', type: 'app' }, { id: 2, name: 'Game', type: 'app' }])?.id).toBe(2);
    expect(CriticScores.normalize("Ghost of Tsushima DIRECTOR’S CUT")).toBe(CriticScores.normalize("Ghost of Tsushima: Director's Cut"));
  });
  it('selects the actual PC critic card instead of headline PS5 score', () => {
    expect(CriticScores.parseMetacritic(criticHtml, title)?.score).toBe(87);
    expect(CriticScores.parseMetacritic(criticHtml, 'Another Game')).toBeNull();
    expect(CriticScores.parseMetacritic(criticHtml, title, 'playstation-4')).toBeNull();
  });
  it('reads platform-specific user overview instead of individual reviews', () => {
    expect(CriticScores.parseMetacriticUser(userHtml, title)?.score).toBe(6.5);
    expect(CriticScores.parseMetacriticUser(userHtml, title, 'playstation-4')).toBeNull();
    expect(CriticScores.parseMetacriticUser(userHtml.replace('User score 6.5', 'User score 11'), title)).toBeNull();
  });
  it('loads PS4 ratings without Steam requests and keeps console cache separate', async () => {
    const storage = memory(); let count = 0;
    const ps4Critic = criticHtml.replaceAll('platform=pc', 'platform=playstation-4');
    const ps4User = userHtml.replaceAll('platform=pc', 'platform=playstation-4').replaceAll('PC User Reviews', 'PlayStation 4 User Reviews');
    const service = CriticScoresService.createService({ storage, platform: 'playstation-4', interval: 0, fetchImpl: async input => {
      const url = String(input); count++;
      expect(url).toContain('www.metacritic.com'); expect(url).toContain('platform=playstation-4');
      return new Response(url.includes('user-reviews') ? ps4User : ps4Critic);
    } });
    const value = await service.lookup({ title });
    expect(value.metacritic?.platform).toBe('PS4');
    expect(value.metacritic?.score).toBe(87); expect(value.metacritic?.userScore).toBe(6.5);
    expect(value.steam).toBeNull(); expect(value.appId).toBeNull();
    await service.lookup({ title }); expect(count).toBe(2);
    expect(Object.keys(storage.data).every(key => key.startsWith('score:v2:ps4:'))).toBe(true);
  });
  it('deduplicates requests, verifies embedded Steam identity and reuses 7-day SQLite-shaped entries', async () => {
    const storage = memory(); let count = 0, now = 1000;
    const service = CriticScoresService.createService({ storage, interval: 0, now: () => now, fetchImpl: async input => {
      const url = String(input); count++;
      if (url.includes('storesearch')) return Response.json({ items: [{ id: 100, name: title, type: 'app' }] });
      if (url.includes('appdetails')) return Response.json({ differentKey: { success: true, data: { steam_appid: 100, type: 'game', name: title, metacritic: { score: 87, url: `https://www.metacritic.com/game/pc/${metaTitle}` } } } });
      if (url.includes('appreviews')) return Response.json({ success: 1, query_summary: { total_positive: 900, total_negative: 100 } });
      return new Response(userHtml);
    } });
    const [a, b] = await Promise.all([service.lookup({ title }), service.lookup({ title })]);
    expect(a).toEqual(b); expect(a.steam?.percent).toBe(90); expect(a.metacritic?.userScore).toBe(6.5);
    expect(a.metacritic?.url).toBe(`https://www.metacritic.com/game/${metaTitle}/?platform=pc`);
    expect(count).toBe(4);
    now += 7 * 86400000 - 1; await service.lookup({ title }); expect(count).toBe(4);
    now++; await service.lookup({ title }); expect(count).toBe(8);
  });
  it('honors a provider 429 cooldown across games without retrying aggressively', async () => {
    const storage = memory(); let count = 0;
    const service = CriticScoresService.createService({ storage, interval: 0, fetchImpl: async () => { count++; return new Response('', { status: 429, headers: { 'Retry-After': '120' } }); } });
    await service.lookup({ title }); const first = count;
    await service.lookup({ title: 'Another game' }); expect(count).toBe(first);
  });
});

describe('rating freshness and offline preservation', () => {
  it('uses 7 days for ratings, one hour for missing ratings, one minute for failures', () => {
    const value = saved();
    expect(ratingsDue(value, 1000 + 7 * 86400000 - 1)).toBe(false);
    expect(ratingsDue(value, 1000 + 7 * 86400000)).toBe(true);
    expect(hasRatings({ ...value, status: 'unverified' })).toBe(false);
    value.steam = null; value.metacritic = null; value.status = 'not-found';
    expect(ratingsDue(value, 3601000)).toBe(true); expect(ratingsDue(value, 61000)).toBe(false);
    value.errors = ['steam-search']; expect(ratingsDue(value, 61000)).toBe(true);
    expect(hasRatings(value)).toBe(false);
  });
  it('retains old verified ratings and their age on failures, without inventing zeros', () => {
    const previous = saved();
    const failed: GameRatings = { ...previous, status: 'not-found', name: null, appId: null, checkedAt: 999999, metacritic: null, steam: null, errors: ['steam-search', 'metacritic'] };
    expect(retainRatings(previous, failed)).toEqual({ ...previous, errors: failed.errors });
    const other = { ...failed, appId: 999, status: 'ok' as const };
    expect(retainRatings(previous, other)).toEqual(other);
  });
});
