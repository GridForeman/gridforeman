import type { ReactNode } from 'react';
import { NavLink } from 'react-router-dom';
import type { AppRoute } from '../appTypes';

type Props = {
  title: string;
  onCreateUser: () => void;
  onCreateBadge: () => void;
  children: ReactNode;
};

const routes: Array<{ to: AppRoute; label: string }> = [
  { to: '/stations', label: 'Colonnine' },
  { to: '/users', label: 'Utenti' },
  { to: '/badges', label: 'Badge' },
  { to: '/events', label: 'Eventi' },
  { to: '/transactions', label: 'Transazioni' },
];

function navClassName({ isActive }: { isActive: boolean }) {
  return `nav-item ${isActive ? 'active' : ''}`;
}

export function AppFrame({ title, onCreateUser, onCreateBadge, children }: Props) {
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">GF</div>
          <div>
            <div className="brand-title">Grid Foreman</div>
            <div className="brand-subtitle">Admin console</div>
          </div>
        </div>

        <nav className="nav">
          {routes.map((route) => (
            <NavLink key={route.to} to={route.to} className={navClassName} end>
              {route.label}
            </NavLink>
          ))}
        </nav>

        <div className="sidebar-footer">
          <div className="status-dot" />
          <div>
            <div className="sidebar-footer-title">Backend</div>
            <div className="sidebar-footer-subtitle">Connesso</div>
          </div>
        </div>
      </aside>

      <main className="main">
        <header className="topbar">
          <div>
            <p className="eyebrow">Admin UI</p>
            <h1>{title}</h1>
          </div>
          <div className="topbar-actions">
            <button className="ghost-button" type="button">
              Esporta
            </button>
            {title === 'Utenti' ? (
              <button className="primary-button" type="button" onClick={onCreateUser}>
                Nuovo utente
              </button>
            ) : title === 'Badge' ? (
              <button className="primary-button" type="button" onClick={onCreateBadge}>
                Nuovo badge
              </button>
            ) : null}
          </div>
        </header>

        {children}
      </main>
    </div>
  );
}
