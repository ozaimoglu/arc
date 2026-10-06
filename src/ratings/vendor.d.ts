import type { GameRatings } from '../types';
export {};
declare global {
  var CriticScores: {
    cleanTitle(input: string): string;
    normalize(input: string): string;
    parseAppId(input: string): number | null;
    chooseMatch(title: string, candidates: unknown[]): { id: number; name: string } | null;
    parseMetacritic(html: string, title: string, platform?: string): GameRatings['metacritic'];
    parseMetacriticUser(html: string, title: string, platform?: string): { score: number; url: string; platform: string } | null;
  };
  var CriticScoresService: {
    createService(options: { storage: { get(key: string): Promise<Record<string, unknown>>; set(values: Record<string, unknown>): Promise<void> }; fetchImpl?: typeof fetch; platform?: string; interval?: number; now?: () => number }): {
      lookup(input: { title: string; appId?: number | null; force?: boolean }): Promise<GameRatings>;
    };
  };
}
