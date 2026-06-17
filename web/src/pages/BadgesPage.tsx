import type { AppActions, AppData } from '../appTypes';

type Props = Pick<AppData, 'badges' | 'users' | 'selectedBadge' | 'loadingBadges' | 'badgeError'> & {
  actions: Pick<AppActions, 'openModal' | 'selectBadge' | 'toggleBadgeActive'>;
};

export function BadgesPage({ badges, users, selectedBadge, loadingBadges, badgeError, actions }: Props) {
  return (
    <section>
      <article className="panel panel-table">
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
                  <th>Azioni</th>
                </tr>
              </thead>
              <tbody>
                {badges.map((badge) => {
                  const owner = badge.user_id == null ? null : users.find((user) => user.id === badge.user_id);
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
                        <span className={`pill ${badge.active ? 'pill-online' : 'pill-error'}`}>
                          {badge.active ? 'Attivo' : 'Disattivato'}
                        </span>
                      </td>
                      <td>
                        <div className="row-actions">
                          <button
                            className="ghost-button small-button"
                            type="button"
                            onClick={(event) => {
                              event.stopPropagation();
                              actions.selectBadge(badge.id);
                              actions.openModal('edit-badge');
                            }}
                          >
                            Modifica
                          </button>
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
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </article>
    </section>
  );
}
