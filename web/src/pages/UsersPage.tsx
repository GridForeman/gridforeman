import type { AppActions, AppData } from '../appTypes';

type Props = Pick<AppData, 'users' | 'loadingUsers' | 'userError'> & {
  actions: Pick<AppActions, 'openModal' | 'selectUser' | 'toggleUserActive'>;
  formatDate: (value: string) => string;
};

export function UsersPage({ users, loadingUsers, userError, actions, formatDate }: Props) {
  return (
    <section>
      <article className="panel panel-table">
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
                  <th>Azioni</th>
                </tr>
              </thead>
              <tbody>
                {users.map((user) => (
                  <tr key={user.id}>
                    <td>{user.id}</td>
                    <td>{user.display_name}</td>
                    <td>{user.email ?? 'n/a'}</td>
                    <td><span className={`pill ${user.active ? 'pill-online' : 'pill-error'}`}>{user.active ? 'Attivo' : 'Disattivo'}</span></td>
                    <td>{formatDate(user.created_at)}</td>
                    <td>
                      <div className="row-actions">
                        <button
                          className="ghost-button small-button"
                          type="button"
                          onClick={() => {
                            actions.selectUser(user.id);
                            actions.openModal('edit-user');
                          }}
                        >
                          Modifica
                        </button>
                        <button
                          className="ghost-button small-button"
                          type="button"
                          onClick={() => void actions.toggleUserActive(user)}
                        >
                          {user.active ? 'Disattiva' : 'Attiva'}
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </article>
    </section>
  );
}
