(function (root) {
  "use strict";
  const C = root.CriticScores;
  const PREFIX = "score:v2:";
  const DAY = 86400000;
  const CACHE_TTL = 7 * DAY;

  function createRequestQueue({ concurrency, spacing, now = Date.now, setTimer = setTimeout, clearTimer = clearTimeout }) {
    const waiting = [];
    let active = 0, lastStart = -Infinity, blockedUntil = 0, timer = null;
    function pump() {
      if (now() < blockedUntil) {
        if (timer !== null) { clearTimer(timer); timer = null; }
        for (const job of waiting.splice(0)) job.reject(new Error("rate-limit"));
        return;
      }
      if (timer !== null) return;
      while (active < concurrency && waiting.length) {
        const delay = spacing - (now() - lastStart);
        if (delay > 0) { timer = setTimer(() => { timer = null; pump(); }, delay); return; }
        const job = waiting.shift();
        active++; lastStart = now();
        Promise.resolve().then(() => {
          if (now() < blockedUntil) throw new Error("rate-limit");
          return job.work();
        }).then(job.resolve, job.reject).finally(() => { active--; pump(); });
      }
    }
    return {
      run(work) { return new Promise((resolve, reject) => { waiting.push({ work, resolve, reject }); pump(); }); },
      blockUntil(deadline) { blockedUntil = Math.max(blockedUntil, deadline); pump(); }
    };
  }

  function cacheExpiry(value) {
    // Keep any verified game with a score for seven days, including partial results.
    // Missing sources can be retried explicitly without silently fetching it again.
    const hasScore = value.status === "ok" && (value.steam || typeof value.metacritic?.score === "number" || typeof value.metacritic?.userScore === "number");
    // A verified upcoming game can return HTTP 200 but no ratings yet. Recheck
    // that empty result on a later visit instead of waiting for the full rating TTL.
    const ttl = hasScore ? CACHE_TTL : value.errors.length ? 60000 : 3600000;
    return value.checkedAt + ttl;
  }

  function pageIdentityCheckedAt(entry) {
    // Before 1.5.3 identity records only stored their 180-day expiry.
    return entry?.checkedAt ?? entry?.expires - 180 * DAY;
  }

  function createService({ storage, fetchImpl = fetch, now = Date.now, interval = 300, timeout = 20000, platform = "pc" }) {
    if (!["pc", "playstation-4"].includes(platform)) throw new Error("invalid-platform");
    const PREFIX = platform === "pc" ? "score:v2:" : "score:v2:ps4:";
    const inFlight = new Map();
    const queues = new Map();
    const gameRequests = new Map();
    const metaRequests = new Map();
    const pageRequests = new Map();
    let cacheGeneration = 0;

    async function readCache(key) {
      const entry = (await storage.get(key))[key];
      if (entry?.ref) return readCache(entry.ref);
      if (entry?.value && cacheExpiry(entry.value) > now()) return entry.value;
      return null;
    }

    function cacheEntries(value, key, pageUrl, requestedAppId) {
      const expires = cacheExpiry(value);
      const canonical = value.status === "ok" && value.appId ? `${PREFIX}app:${value.appId}` : key;
      const updates = { [canonical]: { expires, value } };
      const aliases = new Set([key]);
      if (value.status === "ok") {
        if (!requestedAppId) {
          aliases.add(`${PREFIX}title:${C.normalize(value.title)}`);
          if (pageUrl) aliases.add(`${PREFIX}page:${pageUrl}`);
        }
        if (value.name) aliases.add(`${PREFIX}title:${C.normalize(value.name)}`);
      }
      for (const alias of aliases) if (alias !== canonical) updates[alias] = { ref: canonical, expires };
      return updates;
    }

    async function request(input, format = "json") {
      const url = new URL(input, "https://store.steampowered.com");
      if (url.protocol !== "https:" || !["store.steampowered.com", "byxatab.com", "www.byxatab.com", "www.metacritic.com"].includes(url.hostname)) throw new Error("invalid-source");
      const isMeta = url.hostname === "www.metacritic.com";
      // Both byxatab hostnames share one budget; all tabs share these worker queues.
      const source = url.hostname.replace(/^www\.(?=byxatab\.com$)/, "");
      if (!queues.has(source)) queues.set(source, createRequestQueue({
        concurrency: source === "store.steampowered.com" ? 3 : 2,
        spacing: interval === 0 ? 0 : isMeta ? 900 : interval,
        now
      }));
      const queue = queues.get(source);
      // Each attempt acquires a fresh slot; retries cannot skip spacing or cooldown.
      for (let attempt = 0; ; attempt++) {
        try { return await queue.run(async () => {
          const controller = new AbortController();
          const timer = setTimeout(() => controller.abort(), timeout);
          try {
            const response = await fetchImpl(url.href, { credentials: "omit", redirect: isMeta ? "follow" : "error", signal: controller.signal,
              headers: { Accept: format === "json" ? "application/json" : "text/html" } });
            if (isMeta && response.url && (!C.metacriticGameUrl(response.url) || new URL(response.url).protocol !== "https:")) throw new Error("invalid-redirect");
            if (response.status === 429) {
              const retryAfter = response.headers.get("Retry-After");
              const seconds = Number(retryAfter);
              const delay = Number.isFinite(seconds) ? seconds * 1000 : Date.parse(retryAfter) - now();
              queue.blockUntil(now() + Math.max(60000, Number.isFinite(delay) ? delay : 60000));
              throw new Error("rate-limit");
            }
            if (format === "text" && response.status === 404) return "";
            if (!response.ok) throw new Error(`http-${response.status}`);
            return format === "json" ? await response.json() : await response.text();
          } finally { clearTimeout(timer); }
        }); } catch (error) {
          if (!isMeta || attempt > 0 || /^(?:rate-limit|invalid-|http-(?!5\d\d))/.test(error.message)) throw error;
        }
      }
    }

    async function pageIdentity(pageUrl, force) {
      const key = `${PREFIX}page-id:${pageUrl}`;
      const saved = !force && (await storage.get(key))[key];
      if (pageIdentityCheckedAt(saved) + CACHE_TTL > now()) return saved.appId;
      if (pageRequests.has(pageUrl)) return pageRequests.get(pageUrl);
      const generation = cacheGeneration;
      const task = (async () => {
        const appId = C.pageAppId(await request(pageUrl, "text"));
        const checkedAt = now();
        if (generation === cacheGeneration) await storage.set({ [key]: { appId, checkedAt, expires: checkedAt + CACHE_TTL } });
        return appId;
      })().finally(() => pageRequests.delete(pageUrl));
      pageRequests.set(pageUrl, task);
      return task;
    }

    async function lookup({ title, appId = null, pageUrl = null, force = false }) {
      title = C.cleanTitle(title);
      if (!title || title.length < 2) throw new Error("invalid-title");
      appId = C.parseAppId(appId);
      pageUrl = C.gamePageUrl(pageUrl);
      const requestedAppId = appId;
      let pageFailed = false;
      // A title can belong to multiple Steam games. Confirm the card's own ID before using title aliases.
      if (!appId && pageUrl) {
        try { appId = await pageIdentity(pageUrl, force); }
        catch { pageFailed = true; }
      }
      const key = PREFIX + (appId ? `app:${appId}` : `title:${C.normalize(title)}`);
      const saved = !force ? await readCache(key) || (!appId && pageUrl ? await readCache(`${PREFIX}page:${pageUrl}`) : null) : null;
      if (saved && (saved.metaCheckedAt !== undefined && saved.metaUserCheckedAt !== undefined || saved.status !== "ok")) return saved;
      if (inFlight.has(key)) return inFlight.get(key);
      const generation = cacheGeneration;
      const task = (saved ? Promise.resolve(saved) : resolveGame(title, appId, force, pageFailed, generation)).then(enrichMetacritic).then(async value => {
        if (generation === cacheGeneration) await storage.set(cacheEntries(value, key, pageUrl, requestedAppId));
        return value;
      }).finally(() => inFlight.delete(key));
      inFlight.set(key, task);
      return task;
    }

    async function getMetacritic(title, hint) {
      const names = [...C.titleVariants(title)].reverse();
      const urls = [...new Set([C.metacriticGameUrl(hint), ...names.slice(0, 2).flatMap(C.metacriticSlugs).map(slug => `https://www.metacritic.com/game/${slug}/`)].filter(Boolean))];
      for (const url of urls) {
        const html = await request(`${url}?platform=${platform}`, "text");
        if (!html) continue;
        const page = C.metacriticPage(html, title, platform);
        if (page) return page;
      }
      return null;
    }

    async function getMetacriticUser(title, hint) {
      const known = C.metacriticGameUrl(hint);
      const urls = [...new Set([known, ...[...C.titleVariants(title)].reverse().slice(0, 2).flatMap(C.metacriticSlugs).map(slug => `https://www.metacritic.com/game/${slug}/`)].filter(Boolean))];
      for (const url of urls) {
        const html = await request(`${url}user-reviews/?platform=${platform}`, "text");
        if (!html) continue;
        const user = C.parseMetacriticUser(html, title, platform);
        if (user) return user;
      }
      return null;
    }

    async function enrichMetacritic(value) {
      if (value.metaCheckedAt !== undefined && value.metaUserCheckedAt !== undefined || value.status === "unverified") return value;
      const key = value.appId || C.normalize(value.name || value.title);
      if (metaRequests.has(key)) return { ...await metaRequests.get(key), title: value.title };
      const task = completeMetacritic(value).finally(() => metaRequests.delete(key));
      metaRequests.set(key, task);
      return task;
    }

    async function completeMetacritic(value) {
      const result = { ...value, errors: value.errors.filter(error => !(error === "metacritic" && value.metaCheckedAt === undefined || error === "metacritic-user" && value.metaUserCheckedAt === undefined)), metaCheckedAt: value.metaCheckedAt ?? now(), metaUserCheckedAt: value.metaUserCheckedAt ?? now() };
      if (C.metacriticScore(value.metacritic?.score) === null && value.metaCheckedAt === undefined) {
        try {
          const meta = await getMetacritic(value.name || value.title, value.metacritic?.url || value.metaUrl);
          if (meta) result.metacritic = { ...value.metacritic, ...meta };
        }
        catch { result.errors.push("metacritic"); }
      }
      if (value.metaUserCheckedAt === undefined) {
        try {
          const user = await getMetacriticUser(value.name || value.title, result.metacritic?.url || value.metaUrl);
          if (user) {
            result.metacritic = { ...(result.metacritic || { score: null, url: user.url.replace("user-reviews/", ""), source: "metacritic", platform: platform === "pc" ? "PC" : "PS4" }), userScore: user.score, userUrl: user.url };
          }
        } catch { result.errors.push("metacritic-user"); }
      }
      if (result.metacritic) { result.name ||= result.title; result.status = "ok"; }
      return result;
    }

    async function resolveGame(title, appId, force, pageFailed = false, generation = cacheGeneration) {
      let match = null;
      const result = { title, name: null, appId: null, status: "not-found", metacritic: null, steam: null, errors: pageFailed ? ["game-page"] : [], checkedAt: now() };
      // Arc console entries must never inherit ratings from a PC port.
      if (platform !== "pc") return result;
      if (!appId) {
        try {
          for (const query of C.searchQueries(title)) {
            const search = await request(`/api/storesearch/?term=${encodeURIComponent(query)}&l=english&cc=us`);
            if (!Array.isArray(search?.items)) throw new Error("invalid-search-response");
            match = C.chooseMatch(title, search.items);
            if (match) { appId = C.parseAppId(match.id); break; }
          }
        } catch { result.errors.push("steam-search"); }
      }

      if (appId) {
        if (!force) {
          const cached = await readCache(`${PREFIX}app:${appId}`);
          if (cached) return { ...cached, title };
        }
        // Cards and a detail page can discover the same Steam ID simultaneously.
        if (gameRequests.has(appId)) return { ...await gameRequests.get(appId), title };
        // Hold the shared game task through enrichment and persistence. A second title
        // can arrive after Steam finishes but before Metacritic/cache publication.
        const request = completeGame(result, title, appId, match).then(enrichMetacritic).then(async value => {
          if (generation === cacheGeneration) await storage.set(cacheEntries(value, `${PREFIX}app:${appId}`, null, appId));
          return value;
        });
        gameRequests.set(appId, request);
        try { return await request; } finally { gameRequests.delete(appId); }
      }
      return result;
    }

    async function completeGame(result, title, appId, match) {
      {
        result.appId = appId;
        result.name = match?.name || title;
        result.status = "ok";
        const [details, reviews] = await Promise.allSettled([
          request(`/api/appdetails?appids=${appId}&l=english&cc=us&filters=basic,metacritic`),
          request(`/appreviews/${appId}?json=1&language=all&purchase_type=all&filter=all&num_per_page=0&filter_offtopic_activity=0`)
        ]);
        const data = details.status === "fulfilled" ? C.steamDetails(details.value, appId) : null;
        if (data) {
          if (!match && !["game", "dlc"].includes(data?.type)) return { ...result, appId: null, status: "not-found" };
          result.name = data?.name || result.name;
          const score = C.metacriticScore(data?.metacritic?.score);
          result.metaUrl = C.metacriticGameUrl(data?.metacritic?.url);
          if (score !== null) result.metacritic = { score, url: result.metaUrl ? `${result.metaUrl}?platform=pc` : null, source: "steam", platform: "PC" };
        } else {
          result.errors.push("steam-details");
          if (!match) result.status = "unverified";
        }
        if (!result.metacritic && C.metacriticScore(match?.metascore) !== null) result.metacritic = { score: C.metacriticScore(match.metascore), url: null, source: "steam", platform: "PC" };
        if (reviews.status === "fulfilled" && reviews.value?.success === 1) {
          const summary = reviews.value.query_summary;
          if (summary && [summary.total_positive, summary.total_negative].every(n => Number.isSafeInteger(n) && n >= 0)) result.steam = C.steamRating(summary.total_positive, summary.total_negative);
          else result.errors.push("steam");
        } else result.errors.push("steam");
      }
      return result;
    }

    async function clearCache() {
      cacheGeneration++;
      const all = await storage.get(null);
      const keys = Object.keys(all).filter(key => key.startsWith("score:"));
      if (keys.length) await storage.remove(keys);
    }

    async function pruneCache() {
      const all = await storage.get(null);
      const entries = Object.entries(all).filter(([key]) => key.startsWith("score:"))
        .sort((a, b) => (a[1]?.value?.checkedAt || 0) - (b[1]?.value?.checkedAt || 0));
      const obsolete = [];
      const migrated = {};
      for (const [key, entry] of entries) {
        const isPageIdentity = key.startsWith(`${PREFIX}page-id:`);
        const expires = entry?.value ? cacheExpiry(entry.value) : isPageIdentity ? pageIdentityCheckedAt(entry) + CACHE_TTL : entry?.expires;
        if (!key.startsWith(PREFIX) || !(expires > now())) { obsolete.push(key); continue; }
        if (isPageIdentity) migrated[key] = { ...entry, checkedAt: pageIdentityCheckedAt(entry), expires };
        if (entry.value) {
          const pageUrl = key.startsWith(`${PREFIX}page:`) ? C.gamePageUrl(key.slice(`${PREFIX}page:`.length)) : null;
          const requestedAppId = key.startsWith(`${PREFIX}app:`);
          Object.assign(migrated, cacheEntries(entry.value, key, pageUrl, requestedAppId));
        }
      }
      // Never evict a still-valid result merely because many games were viewed.
      if (Object.keys(migrated).length) await storage.set(migrated);
      const removals = obsolete.filter(key => !Object.hasOwn(migrated, key));
      if (removals.length) await storage.remove(removals);
    }
    return { lookup, clearCache, pruneCache };
  }
  root.CriticScoresService = { createService };
  if (typeof module !== "undefined" && module.exports) module.exports = { createService, createRequestQueue };
})(globalThis);
