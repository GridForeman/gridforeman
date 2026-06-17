import type { ReactNode } from 'react';

type Props = {
  eyebrow: string;
  title: string;
  onClose: () => void;
  children: ReactNode;
};

export function ModalShell({ eyebrow, title, onClose, children }: Props) {
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal-card" onClick={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div>
            <div className="modal-eyebrow">{eyebrow}</div>
            <h2>{title}</h2>
          </div>
        </div>
        {children}
      </div>
    </div>
  );
}
