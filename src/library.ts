import type { Game, Sort, View } from './types';

export function featuredGames(games: Game[], limit = 6): Game[] {
  return games.filter(game => !game.hidden && game.available).sort((a, b) =>
    (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0)
    || Number(b.favorite) - Number(a.favorite)
    || Number(Boolean(b.hero)) - Number(Boolean(a.hero))
    || b.addedAt - a.addedAt
    || a.title.localeCompare(b.title)
  ).slice(0, limit);
}

export function filterGames(games: Game[], view: View, search: string, genre: string, sort: Sort): Game[] {
  const query = search.trim().toLocaleLowerCase();
  return games.filter(game => (
    (view === 'hidden' ? game.hidden : !game.hidden)
    && (view !== 'favorites' || game.favorite)
    && (view !== 'recent' || game.lastPlayed !== null)
    && (!query || game.title.toLocaleLowerCase().includes(query))
    && (genre === 'all' || game.genre === genre)
  )).sort((a, b) => sort === 'title'
    ? a.title.localeCompare(b.title)
    : sort === 'added' ? b.addedAt - a.addedAt : (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0) || a.title.localeCompare(b.title));
}

export function playtimeLabel(minutes: number): string {
  if (minutes === 0) return 'Not played yet';
  return minutes < 60 ? `${minutes}m played` : `${Math.floor(minutes / 60)}h ${minutes % 60}m played`;
}

export function lastPlayedLabel(time: number | null): string {
  if (!time) return 'Ready when you are';
  const days = Math.floor((Date.now() - time) / 86_400_000);
  return days === 0 ? 'Played today' : days === 1 ? 'Played yesterday' : `Played ${days} days ago`;
}
