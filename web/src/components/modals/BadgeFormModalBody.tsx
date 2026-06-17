import type { BadgeFormModalBodyProps } from './modalTypes';

export function BadgeFormModalBody({
  saving,
  formError,
  closeModal,
  handleSave,
  users,
  badgeDraft,
  setBadgeDraft,
}: BadgeFormModalBodyProps) {
  return (
    <form className="stack-form modal-form" onSubmit={handleSave}>
      <label className="field">
        <span>Badge code</span>
        <input
          value={badgeDraft.badge_code}
          onChange={(event) => setBadgeDraft((current) => ({ ...current, badge_code: event.target.value }))}
          placeholder="04A1B2C3D4"
          required
          autoFocus
        />
      </label>
      <label className="field">
        <span>Etichetta</span>
        <input
          value={badgeDraft.label}
          onChange={(event) => setBadgeDraft((current) => ({ ...current, label: event.target.value }))}
          placeholder="Badge ingresso nord"
        />
      </label>
      <label className="field">
        <span>Utente assegnato</span>
        <div className="field-select-shell">
          <select
            className="field-select"
            value={badgeDraft.user_id}
            onChange={(event) => setBadgeDraft((current) => ({ ...current, user_id: event.target.value }))}
          >
            <option value="">Nessuno</option>
            {users.map((user) => (
              <option key={user.id} value={user.id}>
                {user.display_name}
              </option>
            ))}
          </select>
        </div>
      </label>

      {formError ? <div className="empty-state error">{formError}</div> : null}

      <div className="modal-actions">
        <button className="ghost-button" type="button" onClick={closeModal} disabled={saving}>
          Annulla
        </button>
        <button className="primary-button" type="submit" disabled={saving}>
          {saving ? 'Salvataggio...' : 'Conferma'}
        </button>
      </div>
    </form>
  );
}
