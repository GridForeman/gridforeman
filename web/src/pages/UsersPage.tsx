import type { AppActions, AppData } from '../appTypes';

type Props = Pick<AppData, 'users' | 'selectedUser' | 'selectedUserBadges' | 'loadingUsers' | 'userError'> & {
  actions: Pick<AppActions, 'openModal' | 'selectUser' | 'toggleUserActive'>;
  formatDate: (value: string) => string;
};

export function UsersPage({ users, selectedUser, selectedUserBadges, loadingUsers, userError, actions, formatDate }: Props) {
  return (
    <section className="content-grid users-grid">
      <article className="panel">
        <div className="panel-header">
          <div>
            <h2>Utenti</h2>
            <p>Gestione base: crea utenti e abilita/disabilita accessi.</p>
          </div>
        </div>

        {loadingUsers ? (
          <div className="empty-state">Caricamento utenti...</div>
        ) : userError ? (
          <div className="empty-state error">{userError}</div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>ID</th>
                  <th>Nome</th>
                  <th>Email</th>
                  <th>Stato</th>
                  <th>Creato</th>
                  <th>Azione</th>
                </tr>
              </thead>
              <tbody>
                {users.map((user) => (
                  <tr
                    key={user.id}
                    className={user.id === selectedUser?.id ? 'selected-row' : undefined}
                    onClick={() => actions.selectUser(user.id)}
                  >
                    <td>{user.id}</td>
                    <td>{user.display_name}</td>
                    <td>{user.email ?? 'n/a'}</td>
                    <td><span className={`pill ${user.active ? 'pill-online' : 'pill-error'}`}>{user.active ? 'Attivo' : 'Disattivo'}</span></td>
                    <td>{formatDate(user.created_at)}</td>
                    <td>
                      <button
                        className="ghost-button small-button"
                        type="button"
                        onClick={(event) => {
                          event.stopPropagation();
                          void actions.toggleUserActive(user);
                        }}
                      >
                        {user.active ? 'Disattiva' : 'Attiva'}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </article>

      <aside className="panel panel-side">
        <div className="panel-header">
          <div>
            <h2>Dettaglio utente</h2>
            <p>Pronto per badge associati e permessi.</p>
          </div>
        </div>

        {selectedUser ? (
          <>
            <div className="detail-card">
              <div className="detail-title">{selectedUser.display_name}</div>
              <div className="detail-line"><span>ID</span><strong>{selectedUser.id}</strong></div>
              <div className="detail-line"><span>Email</span><strong>{selectedUser.email ?? 'n/a'}</strong></div>
              <div className="detail-line"><span>Stato</span><strong>{selectedUser.active ? 'Attivo' : 'Disattivo'}</strong></div>
              <button
                className="primary-button"
                type="button"
                disabled={!selectedUser}
                onClick={() => actions.openModal('edit-user')}
              >
                Modifica utente
              </button>
            </div>
            <div className="detail-card muted">
              <div className="detail-title">Badge</div>
              {selectedUserBadges.length > 0 ? (
                <div className="badge-list">
                  {selectedUserBadges.map((badge) => (
                    <div key={badge.id} className="badge-item">
                      <div className="badge-item-main">
                        <strong>{badge.badge_code}</strong>
                        <span>{badge.label ?? 'Senza etichetta'}</span>
                      </div>
                      <span className={`pill ${badge.active ? 'pill-online' : 'pill-error'}`}>
                        {badge.active ? 'Attivo' : 'Disattivo'}
                      </span>
                    </div>
                  ))}
                </div>
              ) : (
                <p>Nessun badge collegato a questo utente.</p>
              )}
            </div>
          </>
        ) : (
          <div className="empty-state">Nessun utente selezionato.</div>
        )}
      </aside>
    </section>
  );
}
