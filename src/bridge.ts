import { demoSnapshot } from './demo';
import type { Artwork, ArtworkKind, Game, GameMatch, GameRatings, ScanReport, Settings, Snapshot, ShaderSnapshot } from './types';

type TauriGlobal = { core: { invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>; convertFileSrc(path: string): string }; event: { listen(event: string, handler: () => void): Promise<() => void> } };
declare global { interface Window { __TAURI__?: TauriGlobal } }
export const isDesktop = Boolean(window.__TAURI__);

function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!window.__TAURI__) return Promise.reject(new Error('This action is available in the Arc desktop app.'));
  return window.__TAURI__.core.invoke<T>(command, args);
}

const storageKey = 'arc-preview-v1';
function readPreview(): Snapshot {
  try { const data = localStorage.getItem(storageKey); if (data) return { ...JSON.parse(data) as Snapshot, runningGameIds: [] }; } catch { /* Use fixtures if storage is unavailable. */ }
  return structuredClone(demoSnapshot);
}
let preview = readPreview();
function savePreview() { try { localStorage.setItem(storageKey, JSON.stringify(preview)); } catch { /* Session still works. */ } }

export function imageSrc(path: string | null): string | undefined {
  if (!path) return undefined;
  return /^https?:|^data:|^blob:|^\//.test(path) ? path : window.__TAURI__?.core.convertFileSrc(path);
}

export const api = {
  snapshot: (): Promise<Snapshot> => isDesktop ? invoke('get_library') : Promise.resolve(structuredClone(preview)),
  settings: async (settings: Settings): Promise<void> => {
    if (isDesktop) return invoke('save_settings', { settings });
    preview.settings = settings; savePreview();
  },
  patch: async (id: number, patch: Partial<Pick<Game, 'title' | 'genre' | 'favorite' | 'hidden' | 'description'>>): Promise<void> => {
    if (isDesktop) return invoke('update_game', { id, patch });
    preview.games = preview.games.map(game => game.id === id ? { ...game, ...patch } : game); savePreview();
  },
  scan: (): Promise<ScanReport> => invoke('scan_library'),
  pickFolder: (): Promise<string | null> => invoke('pick_folder'),
  launch: (id: number): Promise<void> => invoke('launch_game', { id }),
  openFolder: (id: number): Promise<void> => invoke('open_game_folder', { id }),
  openArtworkSite: (): Promise<void> => invoke('open_artwork_site'),
  pickShaderTool: (): Promise<string | null> => invoke('pick_shader_tool'),
  openShaderSite: (): Promise<void> => invoke('open_shader_site'),
  shaderState: (id: number): Promise<ShaderSnapshot> => invoke('shader_state', { id }),
  startShaderJob: (id: number, action: 'analyze' | 'compile'): Promise<void> => invoke('start_shader_job', { id, action }),
  stopShaderJob: (id: number): Promise<void> => invoke('stop_shader_job', { id }),
  ratingsRequest: (url: string): Promise<{ body: string; status: number; url: string; retryAfter: string | null }> => invoke('ratings_request', { url }),
  ratingCatalogueTitle: (id: number): Promise<string> => invoke('rating_catalogue_title', { id }),
  ratingAppId: (id: number): Promise<number | null> => invoke('get_rating_app_id', { id }),
  ratingsCacheRead: (key: string): Promise<Record<string, unknown>> => invoke('ratings_cache_read', { key }),
  ratingsCacheWrite: (values: Record<string, unknown>): Promise<void> => invoke('ratings_cache_write', { values }),
  saveRatings: (id: number, title: string, platform: string, value: GameRatings): Promise<void> => invoke('save_game_ratings', { id, title, platform, value }),
  openRatingSource: (url: string): Promise<void> => invoke('open_rating_source', { url }),
  remove: async (id: number): Promise<void> => {
    if (isDesktop) return invoke('remove_game', { id });
    preview.games = preview.games.filter(game => game.id !== id); savePreview();
  },
  searchMetadata: (query: string): Promise<GameMatch[]> => invoke('search_metadata', { query }),
  linkMetadata: (id: number, sgdbId: number): Promise<void> => invoke('link_metadata', { id, sgdbId }),
  artworks: (id: number, kind: ArtworkKind): Promise<Artwork[]> => invoke('get_artworks', { id, kind }),
  artwork: (id: number, kind: ArtworkKind, artworkId: number): Promise<void> => invoke('set_artwork', { id, kind, artworkId }),
  localArtwork: (id: number, kind: ArtworkKind): Promise<boolean> => invoke('import_artwork', { id, kind }),
  subscribe: async (handler: () => void): Promise<() => void> => window.__TAURI__ ? window.__TAURI__.event.listen('library-changed', handler) : () => {},
};
