(function (root) {
  "use strict";
  const DEFAULTS = Object.freeze({ enabled: true });
  const ROMAN = { ii: "2", iii: "3", iv: "4", v: "5", vi: "6", vii: "7", viii: "8", ix: "9", x: "10" };
  const LOOKALIKES = { а: "a", е: "e", о: "o", р: "p", с: "c", у: "y", х: "x", і: "i", А: "A", В: "B", Е: "E", К: "K", М: "M", Н: "H", О: "O", Р: "P", С: "C", Т: "T", Х: "X" };

  function cleanTitle(input) {
    return String(input ?? "").slice(0, 300)
      .replace(/\[[^\]]*\]/g, " ")
      .replace(/\((?:19|20)\d{2}(?:\s*[-–]\s*(?:19|20)\d{2})?\)/g, " ")
      .replace(/\((?:early access|pc|gog|steam|multi\d*)\)/gi, " ")
      .replace(/\s+(?:v(?:ersion|er)?\.?\s*\d|build\s*\d|repack\b|repak\b|репак\b|папка игры\b).*$/iu, "")
      .replace(/\s*\|\s*(?:(?:PC|RePack|GOG).*)?$/i, "")
      .replace(/\s+PC\s*$/i, "")
      .replace(/\s*\+\s*(?:(?:all|все|\d+)\s+)?DLC.*$/i, "")
      .replace(/\s+/g, " ").replace(/^[\s:|–—-]+|[\s:|–—-]+$/g, "").trim();
  }

  function normalize(input) {
    return cleanTitle(input).replace(/[™®©]/g, "").normalize("NFKD").replace(/\p{M}/gu, "")
      .replace(/[\p{L}\p{N}]+/gu, word => /[a-z]/i.test(word) ? word.replace(/[а-еорсуіхАВЕКМНОРСТХ]/g, c => LOOKALIKES[c] || c) : word)
      .toLowerCase().replace(/[™®©'’‘]/g, "").replace(/&/g, " and ")
      .replace(/[^\p{L}\p{N}]+/gu, " ").trim().split(/\s+/)
      .map(word => ROMAN[word] || word).join(" ");
  }

  function searchQueries(title) {
    const variants = titleVariants(title);
    const queries = variants.flatMap(value => {
      const readable = value.replace(/[™®©]/g, "").replace(/[\p{L}\p{N}]+/gu, word => /[a-z]/i.test(word) ? word.replace(/[а-еорсуіхАВЕКМНОРСТХ]/g, c => LOOKALIKES[c] || c) : word);
      return [readable.trim(), normalize(readable)];
    });
    const shortest = normalize(variants.at(-1) || title).split(" ");
    if (shortest.length > 4) queries.push(shortest.slice(0, 3).join(" "));
    return [...new Set(queries.filter(Boolean))].slice(0, 5);
  }

  function metacriticSlug(title) {
    // Keep roman numerals in URLs (Hades II -> hades-ii), unlike name comparison.
    return cleanTitle(title).replace(/[™®©]/g, "").normalize("NFKD").replace(/\p{M}/gu, "")
      .toLowerCase().replace(/[а-еорсуіх]/g, c => LOOKALIKES[c] || c)
      .replace(/['’‘]/g, "").replace(/&/g, " and ").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
  }

  function metacriticSlugs(title) {
    // Publishers use both dotted and joined initialisms in game URLs.
    const joined = String(title).replace(/\b(?:[a-z]\.){2,}(?:[a-z]\b)?/gi, value => value.replaceAll(".", ""));
    return [...new Set([metacriticSlug(title), metacriticSlug(joined)].filter(Boolean))];
  }

  function gamePageUrl(value) {
    if (!isXatabUrl(value)) return null;
    const url = new URL(value);
    if (!url.pathname.startsWith("/games/") || url.port || url.username || url.password || url.href.length > 600) return null;
    url.protocol = "https:";
    url.search = ""; url.hash = "";
    return url.href;
  }

  function siteOrigin(value) {
    try {
      const url = new URL(value);
      return ["https:", "http:"].includes(url.protocol) && !url.username && !url.password ? url.origin : null;
    } catch { return null; }
  }

  function isTorrentUrl(value) {
    if (!siteOrigin(value)) return false;
    const host = new URL(value).hostname;
    return host === "torrentoyunindir.com" || host.endsWith(".torrentoyunindir.com");
  }

  function isBuiltinUrl(value) { return isXatabUrl(value) || isTorrentUrl(value); }

  function siteTitle(value) {
    return cleanTitle(String(value || "").replace(/\s+kapak\s+görseli\s*$/iu, "")
      .replace(/\s+(?:(?:PC|Full|Torrent|Türkçe|Tek\s+Link|Ücretsiz)\s+)*[iİı]ndir(?:\s*[-–|:]\s*.*)?\s*$/iu, "")
      .replace(/\s+(?:free\s+)?download\s*$/i, ""));
  }

  function coverAppId(value, allowLocalName = false) {
    try {
      let url = new URL(value);
      if (allowLocalName && isTorrentUrl(url.href) && url.searchParams.get("src")) url = new URL(url.searchParams.get("src"));
      if (allowLocalName && isTorrentUrl(url.href)) return parseAppId(url.pathname.match(/\/steam-([1-9]\d*)-cover(?:[-.])/i)?.[1]);
      if (url.protocol !== "https:" || !(url.hostname === "steamstatic.com" || url.hostname.endsWith(".steamstatic.com"))) return null;
      return parseAppId(url.pathname.match(/\/(?:steam\/)?apps\/([1-9]\d*)\//)?.[1]);
    } catch { return null; }
  }

  function htmlAttribute(tag, name) {
    return tag.match(new RegExp(`\\b${name}\\s*=\\s*(["'])([\\s\\S]*?)\\1`, "i"))?.[2] || "";
  }

  function pageAppId(html) {
    const markup = html.replace(/<!--[\s\S]*?-->|<script\b[^>]*>[\s\S]*?<\/script>/gi, "");
    const ids = [...markup.matchAll(/<[^!\/][^>]*\bdata-appid\s*=[^>]*>/gi)]
      .filter(([tag]) => htmlAttribute(tag, "class").split(/\s+/).includes("steamrate"))
      .map(([tag]) => parseAppId(htmlAttribute(tag, "data-appid"))).filter(Boolean);
    return new Set(ids).size === 1 ? ids[0] : null;
  }

  function metacriticIdentity(html, title) {
    const expected = titleVariants(title).map(normalize);
    let game;
    for (const [, attributes, json] of html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/gi)) {
      if (htmlAttribute(attributes, "type") !== "application/ld+json") continue;
      try {
        const parsed = JSON.parse(json);
        const objects = Array.isArray(parsed) ? parsed : parsed["@graph"] || [parsed];
        game = objects.find(item => item && [item["@type"]].flat().includes("VideoGame") && expected.includes(normalize(item.name)));
        if (game) break;
      } catch { /* Ignore unrelated/malformed structured data. */ }
    }
    return metacriticGameUrl(game?.url) ? game : null;
  }

  function metacriticGameUrl(value) {
    try {
      const url = new URL(value);
      if (!["http:", "https:"].includes(url.protocol) || !["metacritic.com", "www.metacritic.com"].includes(url.hostname) || url.port || url.username || url.password) return null;
      const slug = url.pathname.match(/^\/game\/(?:pc\/)?([a-z0-9-]+)(?:\/(?:user-reviews|critic-reviews))?\/?$/)?.[1];
      return slug ? `https://www.metacritic.com/game/${slug}/` : null;
    } catch { return null; }
  }

  function metacriticPage(html, title, platform = "pc") {
    const game = metacriticIdentity(html, title);
    if (!game) return null;
    return parseMetacritic(html, title, platform) || { score: null, url: `${metacriticGameUrl(game.url)}?platform=${platform}`, source: "metacritic", platform: platform === "pc" ? "PC" : "PS4", reason: "no-platform-score" };
  }

  function parseMetacritic(html, title, platform = "pc") {
    const game = metacriticIdentity(html, title);
    const url = metacriticGameUrl(game?.url);
    if (!url) return null;
    const gamePath = new URL(url).pathname.replace(/\/$/, "");
    // Headline JSON-LD often describes PS5. Read the PC platform card explicitly.
    for (const [card, attributes] of html.matchAll(/<a\b([^>]*)>[\s\S]*?<\/a>/gi)) {
      if (!htmlAttribute(attributes, "class").split(/\s+/).includes("product-score-card--platform")) continue;
      try {
        const target = new URL(htmlAttribute(attributes, "href").replace(/&amp;/g, "&"), url);
        if (target.origin !== new URL(url).origin || target.pathname.replace(/\/$/, "") !== `${gamePath}/critic-reviews` || target.searchParams.get("platform") !== platform) continue;
        const value = card.match(/(?:title|aria-label)=["']Metascore\s+(\d+)\s+out of 100["']/i)?.[1];
        const score = metacriticScore(value);
        return score === null ? null : { score, url: `${url}?platform=${platform}`, source: "metacritic", platform: platform === "pc" ? "PC" : "PS4" };
      } catch { /* Do not trust unvalidated page links. */ }
    }
    const label = platform === "pc" ? "PC" : "PlayStation 4";
    if ([game.gamePlatform].flat().length === 1 && [game.gamePlatform].flat()[0] === label && Number(game.aggregateRating?.bestRating) === 100) {
      const score = metacriticScore(game.aggregateRating?.ratingValue);
      if (score !== null) return { score, url: `${url}?platform=${platform}`, source: "metacritic", platform: platform === "pc" ? "PC" : "PS4" };
    }
    return null;
  }

  function htmlText(value) {
    const entities = { amp: "&", quot: '"', apos: "'", lt: "<", gt: ">", nbsp: " " };
    return String(value).replace(/<[^>]*>/g, " ").replace(/&(#x[\da-f]+|#\d+|amp|quot|apos|lt|gt|nbsp);/gi, (entity, name) => {
      if (!name.startsWith("#")) return entities[name.toLowerCase()];
      const point = name[1].toLowerCase() === "x" ? parseInt(name.slice(2), 16) : Number(name.slice(1));
      return point > 0 && point <= 0x10ffff ? String.fromCodePoint(point) : entity;
    }).replace(/\s+/g, " ").trim();
  }

  function parseMetacriticUser(html, title, platform = "pc") {
    const markup = html.replace(/<!--[\s\S]*?-->|<(script|style)\b[^>]*>[\s\S]*?<\/\1>/gi, "");
    const pageTitle = htmlText(markup.match(/<title\b[^>]*>([\s\S]*?)<\/title>/i)?.[1] || "")
      .replace(/\s+user reviews\s*[-–]\s*Metacritic\s*$/i, "");
    if (!titleVariants(title).map(normalize).includes(normalize(pageTitle))) return null;
    const heading = htmlText(markup.match(/<h1\b[^>]*>([\s\S]*?)<\/h1>/i)?.[1] || "");
    if (heading !== `${platform === "pc" ? "PC" : "PlayStation 4"} User Reviews`) return null;
    const canonical = [...markup.matchAll(/<link\b[^>]*>/gi)].find(([tag]) => htmlAttribute(tag, "rel") === "canonical");
    let url;
    try {
      url = new URL(htmlAttribute(canonical?.[0] || "", "href").replace(/&amp;/g, "&"));
      if (!safeMetacriticUrl(url.href) || !/^\/game\/[^/]+\/user-reviews\/$/.test(url.pathname) || url.searchParams.get("platform") !== platform) return null;
    } catch { return null; }
    // Read only the PC overview. Later values include individual reviews and other platforms.
    const start = [...markup.matchAll(/<div\b[^>]*>/gi)].find(([tag]) => htmlAttribute(tag, "data-testid") === "score-card-overview");
    if (!start) return null;
    let depth = 0;
    let overview = "";
    const section = markup.slice(start.index);
    for (const tag of section.matchAll(/<\/?div\b[^>]*>/gi)) {
      depth += /^<\//.test(tag[0]) ? -1 : 1;
      if (depth === 0) { overview = section.slice(0, tag.index + tag[0].length); break; }
    }
    const raw = overview.match(/(?:title|aria-label)=["']User score\s+(\d+(?:\.\d+)?)\s+out of 10["']/i)?.[1];
    if (raw === undefined) return null;
    const score = Number(raw);
    if (!Number.isFinite(score) || score < 0 || score > 10) return null;
    return { score, url: url.href, source: "metacritic", platform: platform === "pc" ? "PC" : "PS4" };
  }

  function titleVariants(input) {
    const clean = cleanTitle(input);
    const parts = clean.split(/\s+\/\s+/).sort((a, b) => b.length - a.length);
    const variants = [];
    for (const part of parts) {
      variants.push(part);
      // Only remove retail bundles. Remake/remastered/director's cut identify distinct releases.
      const base = part.replace(/\s*[:–-]?\s+(?:(?:digital\s+)?deluxe|premium|ultimate|gold|complete|goty|game\s+of\s+the\s+year|collector'?s)\s+edition\b.*$/i, "").trim();
      if (base !== part) variants.push(base);
    }
    return [...new Set(variants.filter(Boolean))];
  }

  function chooseMatch(title, candidates) {
    const variants = titleVariants(title).map(normalize);
    const ranked = (Array.isArray(candidates) ? candidates : [])
      .filter(item => item && item.type === "app" && parseAppId(item.id))
      .map(item => {
        let rank = variants.indexOf(normalize(item.name));
        if (rank < 0 && titleVariants(item.name).slice(1).some(name => variants.includes(normalize(name)))) rank = variants.length;
        return { item, rank };
      })
      .filter(match => match.rank >= 0).sort((a, b) => a.rank - b.rank);
    if (!ranked.length) return null;
    const best = ranked.filter(match => match.rank === ranked[0].rank);
    return new Set(best.map(match => String(match.item.id))).size === 1 ? best[0].item : null;
  }

  function parseAppId(value) {
    const text = String(value ?? "").trim();
    if (/^[1-9]\d{0,9}$/.test(text)) return Number(text);
    try {
      const url = new URL(text);
      if (url.protocol !== "https:" || !["store.steampowered.com", "steamdb.info"].includes(url.hostname)) return null;
      const id = url.pathname.match(/^\/app\/([1-9]\d{0,9})(?:\/|$)/)?.[1];
      return id ? Number(id) : null;
    } catch { return null; }
  }

  function steamDetails(response, appId) {
    appId = parseAppId(appId);
    if (!appId || !response || typeof response !== "object" || Array.isArray(response)) return null;
    // Some responses use a different outer key. Only accept an unambiguous
    // embedded identity matching the requested game, never the first result.
    const matches = Object.entries(response).filter(([key, entry]) => {
      const data = entry?.data;
      if (entry?.success !== true || !data || typeof data !== "object" || Array.isArray(data)) return false;
      return data.steam_appid !== undefined ? String(data.steam_appid) === String(appId) : key === String(appId);
    });
    return matches.length === 1 ? matches[0][1].data : null;
  }

  function steamRating(positive, negative) {
    if (![positive, negative].every(n => Number.isSafeInteger(n) && n >= 0)) return null;
    const total = positive + negative;
    if (!Number.isSafeInteger(total) || total === 0) return null;
    const average = positive / total;
    return { positive, negative, total, percent: average * 100 };
  }

  function metacriticScore(value) {
    if (value === null || value === undefined || value === "") return null;
    const score = Number(value);
    return Number.isFinite(score) && score > 0 && score <= 100 ? score : null;
  }

  function safeMetacriticUrl(value) {
    try {
      const url = new URL(value);
      if (["http:", "https:"].includes(url.protocol) && ["metacritic.com", "www.metacritic.com"].includes(url.hostname)) {
        url.protocol = "https:";
        url.search = "";
        return url.href;
      }
    } catch { /* An API field must never create an arbitrary link. */ }
    return null;
  }

  function isXatabUrl(value) {
    try { const url = new URL(value); return ["http:", "https:"].includes(url.protocol) && ["byxatab.com", "www.byxatab.com"].includes(url.hostname); }
    catch { return false; }
  }

  const api = { DEFAULTS, cleanTitle, normalize, titleVariants, searchQueries, metacriticSlug, metacriticSlugs, gamePageUrl, pageAppId, metacriticGameUrl, metacriticPage, parseMetacritic, parseMetacriticUser, chooseMatch, parseAppId, steamDetails, steamRating, metacriticScore, safeMetacriticUrl, isXatabUrl, siteOrigin, isTorrentUrl, isBuiltinUrl, siteTitle, coverAppId };
  root.CriticScores = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(globalThis);
