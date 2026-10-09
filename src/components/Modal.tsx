import { useEffect, useRef, useState, type ReactNode } from 'react';
import { X } from 'lucide-react';
import { getDesktopWindow } from '../bridge';
import TitleBar from './TitleBar';

export default function Modal({ title, children, onClose, wide = false }: { title: string; children: ReactNode; onClose: () => void; wide?: boolean }) {
  const ref = useRef<HTMLDialogElement>(null);
  const [desktop] = useState(() => Boolean(getDesktopWindow()));
  const [windowError, setWindowError] = useState<string | null>(null);
  useEffect(() => {
    const dialog = ref.current!;
    const previous = document.activeElement as HTMLElement | null;
    dialog.showModal();
    return () => { dialog.close(); previous?.focus(); };
  }, []);
  return <dialog ref={ref} className={`modal-host ${desktop ? 'desktop-modal-host' : ''}`} onCancel={event => { event.preventDefault(); onClose(); }} onClick={event => {
    if (event.target !== event.currentTarget) return;
    onClose();
  }} aria-labelledby="modal-title">
    {desktop && <TitleBar onError={setWindowError} />}
    <div className={`modal ${wide ? 'wide' : ''}`}>
      <div className="modal-header"><h2 id="modal-title">{title}</h2><button className="icon-button" aria-label="Close dialog" autoFocus onClick={onClose}><X size={20} /></button></div>
      {windowError && <p className="form-error" role="alert">{windowError}</p>}
      {children}
    </div>
  </dialog>;
}
