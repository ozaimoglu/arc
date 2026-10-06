import type { Game, Snapshot } from './types';

const now = Date.now();
const entries = [
  [1245620, 'ELDEN RING', 'Action RPG', 2538, 1, true, 'Rise, Tarnished. Explore the Lands Between and become the Elden Lord in a vast world full of wonder and peril.'],
  [1091500, 'Cyberpunk 2077', 'Open-world RPG', 1862, 2, true, 'Become a mercenary in Night City, a sprawling metropolis obsessed with power, glamour and body modification.'],
  [1086940, "Baldur’s Gate 3", 'Role-playing', 3204, 3, false, 'Gather your party and return to the Forgotten Realms in a tale of fellowship, betrayal, sacrifice and survival.'],
  [1627720, 'Lies of P', 'Action RPG', 846, 5, false, 'A dark reimagining of Pinocchio. Find your own path through the fallen city of Krat.'],
  [814380, 'Sekiro: Shadows Die Twice', 'Action adventure', 1435, 8, true, 'Carve your own clever path to vengeance in a beautiful, brutal world inspired by Sengoku Japan.'],
  [292030, 'The Witcher 3: Wild Hunt', 'Open-world RPG', 5613, 14, false, 'As Geralt of Rivia, explore a war-torn open world and track down the child of prophecy.'],
  [1145360, 'Hades', 'Roguelike', 721, 20, true, 'Defy the god of the dead as you battle out of the Underworld in this endlessly replayable adventure.'],
  [367520, 'Hollow Knight', 'Metroidvania', 634, 25, false, 'Descend into a vast ruined kingdom of insects and heroes. Explore twisting caverns and befriend bizarre bugs.'],
] as const;

export const demoGames: Game[] = entries.map(([steam, title, genre, playtime, days, favorite, description], index) => ({
  id: index + 1, title, genre, description, playtime, favorite,
  exePath: `D:\\Games\\${title}\\game.exe`, folder: `D:\\Games\\${title}`,
  source: 'Local game', hidden: false, available: true,
  cover: `https://cdn.cloudflare.steamstatic.com/steam/apps/${steam}/library_600x900.jpg`,
  hero: `https://cdn.cloudflare.steamstatic.com/steam/apps/${steam}/library_hero.jpg`,
  logo: null, sgdbId: null, score: 110,
  lastPlayed: now - days * 86_400_000, addedAt: now - (index + 1) * 86400000,
}));

export const demoSnapshot: Snapshot = {
  games: demoGames,
  settings: { folders: [], apiKey: '', displayName: 'Player', autoWatch: true },
};
