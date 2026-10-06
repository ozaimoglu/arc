import './vendor/core.js';
import './vendor/service.js';
import { api } from '../bridge';
import type { Game, GameRatings } from '../types';

export const ratingPlatform = (game: Game) => game.consoleLaunch ? 'playstation-4' : 'pc';
export const ratingIdentity = (game: Game) => `${game.id}:${ratingPlatform(game)}:${game.title}`;
export function hasRatings(value?: GameRatings | null): boolean {
  return !!value && value.status === 'ok' && (value.metacritic?.score != null || value.metacritic?.userScore != null || !!value.steam);
}
export function ratingsDue(value?: GameRatings | null, now = Date.now()): boolean {
  if (!value) return true;
  const ttl = hasRatings(value) ? 7 * 86400000 : value.errors.length ? 60000 : 3600000;
  return value.checkedAt + ttl <= now;
}
// Failed refreshes retain the original readings and their age for offline use.
export function retainRatings(previous: GameRatings | null | undefined, value: GameRatings): GameRatings {
  if (!previous || !hasRatings(previous) || !value.errors.length || previous.appId !== value.appId && value.appId !== null) return value;
  return {
    ...value, name: value.name || previous.name, appId: value.appId || previous.appId,
    status: 'ok', checkedAt: previous.checkedAt,
    steam: value.steam || previous.steam,
    metacritic: value.metacritic || previous.metacritic ? {
      ...previous.metacritic!, ...value.metacritic!,
      score: value.metacritic?.score ?? previous.metacritic?.score ?? null,
      userScore: value.metacritic?.userScore ?? previous.metacritic?.userScore ?? null,
      url: value.metacritic?.url || previous.metacritic?.url || null,
      userUrl: value.metacritic?.userUrl || previous.metacritic?.userUrl,
    } : null,
  };
}

const storage = { get: api.ratingsCacheRead, set: api.ratingsCacheWrite };
const nativeFetch: typeof fetch = async (input, init) => {
  if (init?.signal?.aborted) throw new DOMException('Aborted', 'AbortError');
  const result = await api.ratingsRequest(String(input));
  if (init?.signal?.aborted) throw new DOMException('Aborted', 'AbortError');
  const response = new Response(result.body, { status: result.status, headers: result.retryAfter ? { 'Retry-After': result.retryAfter } : undefined });
  Object.defineProperty(response, 'url', { value: result.url });
  return response;
};
const services = new Map<string, ReturnType<typeof CriticScoresService.createService>>();
const inFlight = new Map<string, Promise<void>>();
export function loadRatings(game: Game, force = false, appId?: number): Promise<void> {
  const identity = ratingIdentity(game);
  if (inFlight.has(identity)) {
    const pending = inFlight.get(identity)!;
    // Publish an explicit correction after any automatic lookup already running.
    if (appId !== undefined) return pending.catch(() => {}).then(() => loadRatings(game, true, appId));
    return pending;
  }
  const platform = ratingPlatform(game);
  let service = services.get(platform);
  if (!service) { service = CriticScoresService.createService({ storage, fetchImpl: nativeFetch, platform }); services.set(platform, service); }
  const selected = service;
  const task = (async () => {
    let verifiedId = appId ?? game.ratings?.appId;
    if (!verifiedId && platform === 'pc') {
      try { verifiedId = await api.ratingAppId(game.id); } catch { /* Title lookup still works. */ }
    }
    let value = await selected.lookup({ title: game.title, force, appId: verifiedId });
    // Reuse the user's linked artwork catalogue only when Steam could not match
    // the scanner title. This preserves remake/remaster release identity.
    if (platform === 'pc' && !value.appId && appId === undefined && game.sgdbId) {
      try {
        const title = await api.ratingCatalogueTitle(game.id);
        if (CriticScores.normalize(title) !== CriticScores.normalize(game.title)) {
          const canonical = await selected.lookup({ title, force });
          if (canonical.appId || !hasRatings(value) && hasRatings(canonical)) value = canonical;
        }
      } catch { /* The original provider result works without an artwork key. */ }
    }
    // A supplied ID whose Steam details could not be verified must not publish
    // review numbers as if the game identity had been confirmed.
    if (value.status !== 'ok') value = { ...value, steam: null, metacritic: null };
    await api.saveRatings(game.id, game.title, platform, retainRatings(appId !== undefined && appId !== game.ratings?.appId ? null : game.ratings, value));
  })().finally(() => inFlight.delete(identity));
  inFlight.set(identity, task);
  return task;
}
