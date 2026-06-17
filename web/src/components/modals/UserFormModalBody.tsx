import type { UserFormModalBodyProps } from './modalTypes';

export function UserFormModalBody({
  saving,
  formError,
  closeModal,
  handleSave,
  userDraft,
  setUserDraft,
}: UserFormModalBodyProps) {
  return (
    <form className="stack-form modal-form" onSubmit={handleSave}>
      <label className="field">
        <span>Nome visualizzato</span>
        <input
          value={userDraft.display_name}
          onChange={(event) => setUserDraft((current) => ({ ...current, display_name: event.target.value }))}
          placeholder="Mario Rossi"
          required
          autoFocus
        />
      </label>
      <label className="field">
        <span>Email</span>
        <input
          value={userDraft.email}
          onChange={(event) => setUserDraft((current) => ({ ...current, email: event.target.value }))}
          placeholder="mario@example.com"
          type="email"
        />
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
