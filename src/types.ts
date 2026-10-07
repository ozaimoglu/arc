export interface Game {
  id: number;
  title: string;
  exePath: string;
  folder: string;
  source: string;
  genre: string;
  description: string;
  cover: string | null;
  hero: string | null;
  logo: string | null;
  sgdbId: number | null;
  favorite: boolean;
  hidden: boolean;
  available: boolean;
  lastPlayed: number | null;
  playtime: number;
  score: number;
  addedAt: number;
  consoleLaunch?: { kind: 'shadPs4' | 'bloodborneLauncher'; emulator: string; launcher?: string; titleId: string } | null;
  ratings?: GameRatings | null;
}

export interface GameRatings {
  title: string; name: string | null; appId: number | null;
  status: 'ok' | 'not-found' | 'unverified';
  metacritic: { score: number | null; userScore?: number | null; url: string | null; userUrl?: string | null; source: 'steam' | 'metacritic'; platform: 'PC' | 'PS4'; reason?: string | null } | null;
  steam: { positive: number; negative: number; total: number; percent: number } | null;
  errors: string[]; checkedAt: number; metaCheckedAt?: number | null; metaUserCheckedAt?: number | null; metaUrl?: string | null;
}

export interface Settings { folders: string[]; apiKey: string; displayName: string; autoWatch: boolean; shaderTool?: string }
export interface ShaderGameStatus {
  id: string; status: string; reason: string; engine: string | null; graphicsApi: string | null;
  antiCheat: string; shaderCount: number | null; warmedAt: string | null; driver: string | null; canCompile: boolean;
}
export interface ShaderJob {
  gameId: number; title: string; action: 'analyze' | 'compile'; running: boolean;
  phase: string; lines: string[]; error: string | null; stopped: boolean;
}
export interface ShaderSnapshot { installed: boolean; game: ShaderGameStatus | null; job: ShaderJob | null; busy: boolean; warning?: string | null }
export interface Snapshot { games: Game[]; settings: Settings; runningGameIds: number[] }
export interface ScanReport { added: number; updated: number; ignored: number; warnings: string[] }
export type ArtworkKind = 'grid' | 'hero' | 'logo';
export interface Artwork { id: number; url: string; thumb: string; author: string }
export interface GameMatch { id: number; name: string; releaseDate?: number | null }
export type View = 'library' | 'favorites' | 'recent' | 'hidden';
export type Sort = 'recent' | 'title' | 'added';
