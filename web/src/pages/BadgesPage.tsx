import type { AppActions, AppData } from '../appTypes';

type Props = Pick<AppData, 'badges' | 'users' | 'selectedBadge' | 'loadingBadges' | 'badgeError'> & {
  actions: Pick<AppActions, 'openModal' | 'selectBadge' | 'toggleBadgeActive'>;
};

function getAuthorizeStatusText(badge: { active: boolean; user_id: number | null; badge_code: string }) {
  if (!badge.badge_code.trim()) return 'Invalid';
  if (!badge.active || badge.user_id == null) return 'Blocked';
  return 'Accepted';
}

export function BadgesPage({ badges, users, selectedBadge, loadingBadges, badgeError, actions }: Props) {
  return (
    <section className="content-grid users-grid">
      <article className="panel">
        <div className="panel-header">
          <div>
            <h2>Badge</h2>
            <p>Assegna i badge agli utenti e attivali o disattivali da qui.</p>
          </div>
        </div>

        {loadingBadges ? (
          <div className="empty-state">Caricamento badge...</div>
        ) : badgeError ? (
          <div className="empty-state error">{badgeError}</div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>ID</th>
                  <th>Codice</th>
                  <th>Etichetta</th>
                  <th>Utente</th>
                  <th>Stato</th>
                  <th>Azione</th>
                </tr>
              </thead>
              <tbody>
                {badges.map((badge) => {
                  const owner = badge.user_id == null ? null : users.find((user) => user.id === badge.user_id);
                  const authorizeStatus = getAuthorizeStatusText(badge);
                  return (
                    <tr
                      key={badge.id}
                      className={badge.id === selectedBadge?.id ? 'selected-row' : undefined}
                      onClick={() => actions.selectBadge(badge.id)}
                    >
                      <td>{badge.id}</td>
                      <td>{badge.badge_code}</td>
                      <td>{badge.label ?? 'n/a'}</td>
                      <td>{owner?.display_name ?? 'Nessuno'}</td>
                      <td>
                        <span className={`pill ${authorizeStatus === 'Accepted' ? 'pill-online' : 'pill-error'}`}>
                          {authorizeStatus}
                        </span>
                      </td>
                      <td>
                        <button
                          className="ghost-button small-button"
                          type="button"
                          onClick={(event) => {
                            event.stopPropagation();
                            void actions.toggleBadgeActive(badge);
                          }}
                        >
                          {badge.active ? 'Disattiva' : 'Attiva'}
                        </button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </article>

      <aside className="panel panel-side">
        <div className="panel-header">
          <div>
            <h2>Dettaglio badge</h2>
          </div>
        </div>

        {selectedBadge ? (
          <div className="detail-card">
            <div className="detail-title">{selectedBadge.badge_code}</div>
            <div className="detail-line"><span>ID</span><strong>{selectedBadge.id}</strong></div>
            <div className="detail-line">
              <span>Utente</span>
              <strong>
                {selectedBadge.user_id == null
                  ? 'Nessuno'
                  : users.find((user) => user.id === selectedBadge.user_id)?.display_name ?? `Utente #${selectedBadge.user_id}`}
              </strong>
            </div>
            <div className="detail-line">
              <span>Stato</span>
              <strong>{getAuthorizeStatusText(selectedBadge)}</strong>
            </div>
            <button
              className="primary-button"
              type="button"
              disabled={!selectedBadge}
              onClick={() => actions.openModal('edit-badge')}
            >
              Modifica badge
            </button>
          </div>
        ) : (
          <div className="empty-state">Nessun badge selezionato.</div>
        )}
      </aside>
    </section>
  );
}
