import { useEffect, useRef, useState } from 'react';
import { isDesktop } from '../bridge';
import type { Game } from '../types';
import { hasRatings, loadRatings, ratingIdentity, ratingsDue, ratingsExpireAt } from './client';

const FAILED_REFRESH_RETRY_MS = 15 * 60 * 1000;

export function useRatings(games: Game[], priorityId: number | null) {
  const current = useRef(games); current.current = games;
  const priority = useRef(priorityId); priority.current = priorityId;
  const nextAttempt = useRef(new Map<string, number>());
  const wake = useRef<(() => void) | null>(null);
  const [busy, setBusy] = useState(false);
  const identities = games.filter(game => !game.hidden).map(ratingIdentity).join('|');
  const freshness = games.filter(game => !game.hidden).map(game => `${ratingIdentity(game)}:${game.ratings?.checkedAt}:${game.ratings?.status}:${hasRatings(game.ratings)}:${game.ratings?.errors.length}`).join('|');
  useEffect(() => {
    if (!isDesktop || !identities) { setBusy(false); return; }
    let active = true;
    let running = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const retained = new Set(current.current.map(ratingIdentity));
    for (const identity of nextAttempt.current.keys()) if (!retained.has(identity)) nextAttempt.current.delete(identity);
    function dueAt(game: Game) {
      return Math.max(game.ratings ? ratingsExpireAt(game.ratings) : 0, nextAttempt.current.get(ratingIdentity(game)) ?? 0);
    }
    function schedule() {
      const visible = current.current.filter(game => !game.hidden);
      if (!active || !visible.length) return;
      const next = visible.reduce((earliest, game) => Math.min(earliest, dueAt(game)), Infinity);
      timer = setTimeout(kick, Math.max(1, Math.min(next - Date.now(), 2147483647)));
    }
    async function run() {
      running = true;
      try {
        while (active) {
          const now = Date.now();
          const pending = current.current.filter(game => !game.hidden && dueAt(game) <= now);
          const game = pending.find(item => item.id === priority.current) || pending[0];
          if (!game) break;
          const identity = ratingIdentity(game);
          nextAttempt.current.set(identity, now + FAILED_REFRESH_RETRY_MS);
          setBusy(true);
          try {
            const value = await loadRatings(game);
            nextAttempt.current.set(identity, ratingsDue(value) ? Date.now() + FAILED_REFRESH_RETRY_MS : ratingsExpireAt(value));
          } catch { /* Keep cached scores; retry later or through the detail view. */ }
        }
      } finally {
        running = false;
        if (active) { setBusy(false); schedule(); }
      }
    }
    function kick() {
      if (!active) return;
      clearTimeout(timer);
      if (!running) void run();
    }
    const visibleAgain = () => { if (document.visibilityState === 'visible') kick(); };
    wake.current = kick;
    window.addEventListener('focus', kick);
    window.addEventListener('online', kick);
    document.addEventListener('visibilitychange', visibleAgain);
    kick();
    return () => {
      active = false;
      clearTimeout(timer);
      window.removeEventListener('focus', kick);
      window.removeEventListener('online', kick);
      document.removeEventListener('visibilitychange', visibleAgain);
      if (wake.current === kick) wake.current = null;
    };
  }, [identities]);
  useEffect(() => { wake.current?.(); }, [freshness]);
  return busy;
}
