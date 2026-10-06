import { useState } from 'react';
import { ArrowUpRight, RefreshCw } from 'lucide-react';
import { api, isDesktop } from '../bridge';
import { hasRatings, loadRatings } from '../ratings/client';
import type { Game } from '../types';

// Use CriticPeek's score bands on each reading's own scale.
function ratingTone(score: number | null | undefined, scale = 100) {
  if (score == null || !Number.isFinite(score) || score < 0 || score > scale) return 'unavailable';
  const percent = score / scale * 100;
  return percent >= 75 ? 'good' : percent >= 50 ? 'mixed' : 'low';
}

export function RatingSummary({ game }: { game: Game }) {
  const value = game.ratings;
  if (!hasRatings(value)) return null;
  const meta = value?.metacritic;
  return <span className="card-ratings">
    {(meta?.score != null || meta?.userScore != null) && <span className="card-mc-ratings">
      MC <b className="rating-value" data-tone={ratingTone(meta.score)} title={`Metacritic critics · ${meta.platform} · ${meta.score != null ? `${meta.score}/100` : 'unavailable'}`} aria-label={`Metacritic critics: ${meta.score != null ? `${meta.score} out of 100` : 'unavailable'}, ${meta.platform}`}>{meta.score ?? '—'}</b><span aria-hidden="true">/</span><b className="rating-value" data-tone={ratingTone(meta.userScore, 10)} title={`Metacritic users · ${meta.platform} · ${meta.userScore != null ? `${meta.userScore.toFixed(1)}/10` : 'unavailable'}`} aria-label={`Metacritic users: ${meta.userScore != null ? `${meta.userScore.toFixed(1)} out of 10` : 'unavailable'}, ${meta.platform}`}>{meta.userScore != null ? meta.userScore.toFixed(1) : '—'}</b>
    </span>}
    {value?.steam && <>{(meta?.score != null || meta?.userScore != null) && <span className="card-rating-separator" aria-hidden="true"> · </span>}<span title={`${Math.round(value.steam.percent)}% positive · ${value.steam.total.toLocaleString()} Steam reviews · all languages and purchase types, including off-topic activity`} aria-label={`Steam: ${Math.round(value.steam.percent)} percent positive reviews`}>Steam <b className="rating-value" data-tone={ratingTone(Math.round(value.steam.percent))}>{Math.round(value.steam.percent)}</b></span></>}
  </span>;
}

export default function GameRatings({ game }: { game: Game }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [match, setMatch] = useState('');
  const value = game.ratings;
  const platform = game.consoleLaunch ? 'PS4' : 'PC';
  const meta = value?.metacritic;
  async function refresh() {
    setBusy(true); setError('');
    try { await loadRatings(game, true); } catch { setError('Ratings could not be updated. Try again later.'); } finally { setBusy(false); }
  }
  function source(url: string | null | undefined) {
    if (url) void api.openRatingSource(url).catch(() => setError('Could not open the rating source.'));
  }
  async function correctMatch(event: React.FormEvent) {
    event.preventDefault();
    const appId = CriticScores.parseAppId(match);
    if (!appId) { setError('Enter a Steam game URL or numeric App ID.'); return; }
    setBusy(true); setError('');
    try { await loadRatings(game, true, appId); setMatch(''); } catch { setError('The game match could not be updated. Try again later.'); } finally { setBusy(false); }
  }
  return <section className="game-ratings" aria-labelledby="ratings-heading">
    <div className="ratings-heading"><h2 id="ratings-heading">Ratings</h2><button className="text-button" onClick={() => void refresh()} disabled={busy || !isDesktop}><RefreshCw size={14} className={busy ? 'spin' : ''} />{busy ? 'Updating…' : 'Refresh ratings'}</button></div>
    <div className="ratings-row">
      <div className="rating-reading"><span className="rating-label">Metacritic <span>Critics · {platform}</span></span><div className="rating-number rating-value" data-tone={ratingTone(meta?.score)}>{meta?.score ?? '—'}<small>/100</small></div><button className="rating-source" disabled={!meta?.url} onClick={() => source(meta?.url)}>Critic reviews <ArrowUpRight size={13} /></button></div>
      <div className="rating-reading"><span className="rating-label">Metacritic <span>Users · {platform}</span></span><div className="rating-number rating-value" data-tone={ratingTone(meta?.userScore, 10)}>{meta?.userScore != null ? meta.userScore.toFixed(1) : '—'}<small>/10</small></div><button className="rating-source" disabled={!meta?.userUrl} onClick={() => source(meta?.userUrl)}>User reviews <ArrowUpRight size={13} /></button></div>
      {!game.consoleLaunch && <div className="rating-reading"><span className="rating-label">Steam <span>Positive reviews</span></span><div className="rating-number rating-value" data-tone={ratingTone(value?.steam ? Math.round(value.steam.percent) : null)}>{value?.steam ? Math.round(value.steam.percent) : '—'}<small>%</small></div><button className="rating-source" disabled={!value?.appId || !value?.steam} onClick={() => source(`https://store.steampowered.com/app/${value?.appId}/`)}>{value?.steam ? `${value.steam.total.toLocaleString()} reviews` : 'Steam reviews'} <ArrowUpRight size={13} /></button></div>}
    </div>
    <p className="ratings-note">{!value ? 'Ratings are loaded in the background.' : hasRatings(value) ? `Checked ${new Date(value.checkedAt).toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })}.` : value.status === 'unverified' ? 'Game identity could not be verified. Refresh or correct the match.' : value.status === 'not-found' ? 'No verified ratings found for this title.' : 'This game has no published ratings yet.'} {value?.errors.length ? 'Some sources are unavailable. Saved ratings are kept.' : '— means unavailable.'}</p>
    {value?.steam && <p className="ratings-definition">Steam: all languages and purchase types, including off-topic reviews.</p>}
    {value?.name && value.name !== game.title && <p className="ratings-definition">Matched to {value.name}</p>}
    {!game.consoleLaunch && <details className="ratings-match"><summary>Correct game match</summary><form onSubmit={event => void correctMatch(event)}><label htmlFor="rating-match">Steam game URL or App ID</label><div className="rating-match-input"><input id="rating-match" value={match} onChange={event => setMatch(event.target.value)} placeholder="https://store.steampowered.com/app/…" maxLength={300} /><button className="secondary-button" disabled={busy || !isDesktop || !match.trim()}>Use game</button></div><p>Choose the release you installed. Your verified match is kept for future updates.</p></form></details>}
    {error && <p className="ratings-error" role="alert">{error}</p>}
  </section>;
}
