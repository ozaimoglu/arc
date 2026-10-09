import { useEffect, useRef, useState } from 'react';
import { Copy, Minus, Square, X } from 'lucide-react';
import { getDesktopWindow } from '../bridge';

export default function TitleBar({ onError }: { onError: (message: string) => void }) {
  const [appWindow] = useState(getDesktopWindow);
  const [maximized, setMaximized] = useState(false);
  const [focused, setFocused] = useState(true);
  const errorRef = useRef(onError);
  errorRef.current = onError;

  useEffect(() => {
    if (!appWindow) return;
    let active = true;
    let resizeRequest = 0;
    let focusRequest = 0;
    const unlisten: Array<() => void> = [];
    const report = (error: unknown) => { if (active) errorRef.current(`Window controls: ${String(error)}`); };
    const retain = (unsubscribe: () => void) => { if (active) unlisten.push(unsubscribe); else unsubscribe(); };
    const syncMaximized = () => {
      const request = ++resizeRequest;
      void appWindow.isMaximized().then(value => {
        if (active && request === resizeRequest) setMaximized(value);
      }).catch(report);
    };
    syncMaximized();
    const initialFocusRequest = focusRequest;
    void appWindow.isFocused().then(value => {
      if (active && initialFocusRequest === focusRequest) setFocused(value);
    }).catch(report);
    void appWindow.onResized(syncMaximized).then(retain).catch(report);
    void appWindow.onFocusChanged(event => {
      focusRequest++;
      if (active) setFocused(event.payload);
    }).then(retain).catch(report);
    return () => { active = false; unlisten.forEach(unsubscribe => unsubscribe()); };
  }, [appWindow]);

  if (!appWindow) return null;
  async function control(action: 'minimize' | 'toggleMaximize' | 'close') {
    try { await appWindow![action](); }
    catch (error) { errorRef.current(`Window controls: ${String(error)}`); }
  }

  return <div className={`window-titlebar ${focused ? '' : 'inactive'}`}>
    <div className="window-drag-region" data-tauri-drag-region>
      <img src="/arc-mark.svg" width="16" height="16" alt="" draggable={false} />
      <span>Arc</span>
    </div>
    <div className="window-controls" role="group" aria-label="Window controls">
      <button type="button" className="window-button" aria-label="Minimize window" title="Minimize" onClick={() => void control('minimize')}><Minus size={14} aria-hidden="true" /></button>
      <button type="button" className="window-button" aria-label={maximized ? 'Restore window' : 'Maximize window'} title={maximized ? 'Restore' : 'Maximize'} onClick={() => void control('toggleMaximize')}>
        {maximized ? <Copy size={12} aria-hidden="true" /> : <Square size={12} aria-hidden="true" />}
      </button>
      <button type="button" className="window-button window-close" aria-label="Close Arc" title="Close" onClick={() => void control('close')}><X size={15} aria-hidden="true" /></button>
    </div>
  </div>;
}
