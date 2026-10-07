import { useEffect, useRef } from 'react';
import { Play, Heart, Image, FolderOpen, Pencil, EyeOff, Eye, Trash2 } from 'lucide-react';
import type { Game } from '../types';

export default function ContextMenu({ game, playDisabled, playLabel, x, y, onAction, onClose }: { game: Game; playDisabled: boolean; playLabel: string; x: number; y: number; onAction: (action: string, game: Game) => void; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    ref.current?.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
    function outside(event: PointerEvent) { if (!ref.current?.contains(event.target as Node)) onClose(); }
    document.addEventListener('pointerdown', outside);
    return () => { document.removeEventListener('pointerdown', outside); previous?.focus(); };
  }, [onClose]);
  const actions = [
    ['play', Play, playLabel], ['favorite', Heart, game.favorite ? 'Remove favorite' : 'Add to favorites'],
    ['artwork', Image, 'Change artwork'], ['folder', FolderOpen, 'Open game folder'],
    ['edit', Pencil, 'Game properties'], ['hide', game.hidden ? Eye : EyeOff, game.hidden ? 'Show in library' : 'Hide game'],
    ['remove', Trash2, 'Remove from library'],
  ] as const;
  return <div ref={ref} className="context-menu" role="menu" aria-label={`Actions for ${game.title}`} style={{ left: Math.max(8, Math.min(x, window.innerWidth - 252)), top: Math.max(8, Math.min(y, window.innerHeight - 324)) }} onKeyDown={event => {
    if (event.key === 'Escape' || event.key === 'Tab') { onClose(); return; }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const buttons = [...ref.current!.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];
      const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
      buttons[(index + (event.key === 'ArrowDown' ? 1 : buttons.length - 1)) % buttons.length].focus();
    }
  }}>{actions.map(([action, Icon, label]) => <button role="menuitem" key={action} disabled={action === 'play' && playDisabled} className={action === 'remove' ? 'danger' : ''} onClick={() => { onClose(); onAction(action, game); }}><Icon size={16} />{label}</button>)}</div>;
}
