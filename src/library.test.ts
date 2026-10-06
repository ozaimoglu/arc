import { describe, expect, it } from 'vitest';
import { featuredGames, filterGames, lastPlayedLabel, playtimeLabel } from './library';
import { demoGames } from './demo';

describe('library navigation', () => {
  it('keeps hidden favorites out of visible and recent views', () => {
    const games = demoGames.map(game => ({ ...game, hidden: game.id === 1 }));
    for (const view of ['library', 'favorites', 'recent'] as const) expect(filterGames(games, view, '', 'all', 'recent').some(game => game.id === 1)).toBe(false);
    expect(filterGames(games, 'hidden', '', 'all', 'recent').map(game => game.id)).toEqual([1]);
  });
  it('combines search, genre and favorite filters without mutating the library', () => {
    const before = structuredClone(demoGames);
    expect(filterGames(demoGames, 'favorites', '  ELDEN  ', 'Action RPG', 'title').map(game => game.id)).toEqual([1]);
    expect(filterGames(demoGames, 'library', 'ELDEN', 'Roguelike', 'title')).toEqual([]);
    expect(demoGames).toEqual(before);
  });
  it('never treats an unplayed game as recently played', () => {
    const games = demoGames.map(game => ({ ...game, lastPlayed: game.id === 1 ? null : game.lastPlayed }));
    expect(filterGames(games, 'recent', '', 'all', 'recent').some(game => game.id === 1)).toBe(false);
    expect(filterGames(games, 'library', '', 'all', 'recent').at(-1)?.id).toBe(1);
  });
  it('sorts additions independently of play history', () => {
    const games = demoGames.map(game => ({ ...game, addedAt: game.id }));
    expect(filterGames(games, 'library', '', 'all', 'added').map(game => game.id)).toEqual([8, 7, 6, 5, 4, 3, 2, 1]);
  });
});
describe('featured games', () => {
  it('excludes hidden and unavailable games from the Play spotlight', () => {
    const games = demoGames.map(game => ({ ...game, hidden: game.id === 1, available: game.id !== 2 }));
    expect(featuredGames(games).some(game => game.id === 1 || game.id === 2)).toBe(false);
    expect(featuredGames(games)).toHaveLength(6);
  });
  it('prioritizes play history, then favorites, then hero artwork without duplicates', () => {
    const games: typeof demoGames = demoGames.slice(0, 4).map(game => ({ ...game, lastPlayed: null, favorite: false, hero: null }));
    games[0].lastPlayed = 100;
    games[1].favorite = true;
    games[2].hero = 'hero.webp';
    expect(featuredGames(games).map(game => game.id)).toEqual([1, 2, 3, 4]);
  });
  it('handles an empty library and keeps the source order untouched', () => {
    const before = structuredClone(demoGames);
    expect(featuredGames([])).toEqual([]);
    expect(featuredGames(demoGames, 1)).toHaveLength(1);
    expect(demoGames).toEqual(before);
  });
});
describe('session labels', () => {
  it('distinguishes unplayed games from short sessions', () => {
    expect(playtimeLabel(0)).toBe('Not played yet');
    expect(playtimeLabel(18)).toBe('18m played');
    expect(playtimeLabel(2538)).toBe('42h 18m played');
    expect(lastPlayedLabel(null)).toBe('Ready when you are');
  });
});
