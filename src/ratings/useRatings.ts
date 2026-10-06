import { useEffect, useRef, useState } from 'react';
import { isDesktop } from '../bridge';
import type { Game } from '../types';
import { loadRatings, ratingIdentity, ratingsDue } from './client';

export function useRatings(games: Game[], priorityId: number | null) {
  const current = useRef(games); current.current = games;
  const priority = useRef(priorityId); priority.current = priorityId;
  const attempted = useRef(new Set<string>());
  const [busy, setBusy] = useState(false);
  const identities = games.filter(game => !game.hidden).map(ratingIdentity).join('|');
  useEffect(() => {
    if (!isDesktop || !identities) return;
    let active = true;
    async function run() {
      while (active) {
        const pending = current.current.filter(game => !game.hidden && !attempted.current.has(ratingIdentity(game)) && ratingsDue(game.ratings));
        const game = pending.find(item => item.id === priority.current) || pending[0];
        if (!game) break;
        attempted.current.add(ratingIdentity(game));
        setBusy(true);
        try { await loadRatings(game); } catch { /* Detail view offers an explicit retry. */ }
      }
      if (active) setBusy(false);
    }
    void run();
    return () => { active = false; };
  }, [identities]);
  return busy;
}
