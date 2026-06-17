import type { Dispatch, FormEvent, SetStateAction } from 'react';
import type { ConnectorSummary, StationSummary, User } from '../api';
import type { ModalKind } from '../appTypes';

type Props = {
  modalKind: ModalKind;
  selectedStation: StationSummary | null;
  selectedStationConnectors: ConnectorSummary[];
  saving: boolean;
  stationCommandBusy: boolean;
  formError: string | null;
  loadingStationConnectors: boolean;
  stationConnectorsError: string | null;
  users: User[];
  userDraft: { display_name: string; email: string };
  badgeDraft: { badge_code: string; label: string; user_id: string };
  stationDraft: {
    station_name: string;
    latitude: string;
    longitude: string;
    location_label: string;
    address: string;
    notes: string;
  };
  closeModal: () => void;
  handleSave: (event: FormEvent<HTMLFormElement>) => void;
  refreshStationStatus: (stationId: string) => Promise<void>;
  toggleStationBlocked: (stationId: string, blocked: boolean) => Promise<void>;
  toggleStationConnectorActive: (stationId: string, connectorId: number, active: boolean) => Promise<void>;
  unlockStationConnector: (stationId: string, connectorId: number) => Promise<void>;
  setUserDraft: Dispatch<SetStateAction<{ display_name: string; email: string }>>;
  setBadgeDraft: Dispatch<SetStateAction<{ badge_code: string; label: string; user_id: string }>>;
  setStationDraft: Dispatch<
    SetStateAction<{
      station_name: string;
      latitude: string;
      longitude: string;
      location_label: string;
      address: string;
      notes: string;
    }>
  >;
};

export function AppModal({
  modalKind,
  selectedStation,
  selectedStationConnectors,
  saving,
  stationCommandBusy,
  formError,
  loadingStationConnectors,
  stationConnectorsError,
  users,
  userDraft,
  badgeDraft,
  stationDraft,
  closeModal,
  handleSave,
  refreshStationStatus,
  toggleStationBlocked,
  toggleStationConnectorActive,
  unlockStationConnector,
  setUserDraft,
  setBadgeDraft,
  setStationDraft,
}: Props) {
  if (!modalKind) return null;

  const stationControls = modalKind === 'station-controls';
  const stationControlDisabled = stationCommandBusy || saving || !selectedStation;
  const connectorActionDisabled = !selectedStation || loadingStationConnectors;
  const stationLabel = selectedStation?.station_name?.trim() || selectedStation?.station_id || 'Nessuna colonnina selezionata';

  return (
    <div className="modal-backdrop" onClick={closeModal}>
      <div className="modal-card" onClick={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div>
            <div className="modal-eyebrow">
              {stationControls ? 'Comandi' : modalKind?.startsWith('edit') ? 'Modifica' : 'Nuovo'}
            </div>
            <h2>
              {modalKind === 'create-user'
                ? 'Nuovo utente'
                : modalKind === 'edit-user'
                  ? 'Modifica utente'
                  : modalKind === 'create-badge'
                    ? 'Nuovo badge'
                    : modalKind === 'edit-badge'
                      ? 'Modifica badge'
                      : modalKind === 'station-controls'
                        ? 'Gestione colonnina'
                        : 'Posizione colonnina'}
            </h2>
          </div>
          <button className="ghost-button" type="button" onClick={closeModal} disabled={saving || stationCommandBusy}>
            Chiudi
          </button>
        </div>

        {stationControls ? (
          <div className="stack-form modal-form">
            <div className="modal-readonly">
              <div className="detail-title">{stationLabel}</div>
              <p>Qui trovi i comandi operativi sulla colonnina selezionata.</p>
            </div>
            <div className="detail-line">
              <span>Stato blocco</span>
              <strong>{selectedStation?.blocked ? 'Bloccata' : 'Attiva'}</strong>
            </div>
            <div className="modal-readonly">
              <div className="detail-title">Connettori</div>
              <p>Ogni connettore ha stato e comandi separati.</p>
            </div>
            {loadingStationConnectors ? (
              <div className="empty-state">Caricamento connettori...</div>
            ) : stationConnectorsError ? (
              <div className="empty-state error">{stationConnectorsError}</div>
            ) : selectedStationConnectors.length > 0 ? (
              <div className="stack-form">
                {selectedStationConnectors.map((connector) => {
                  const requiresEvse = selectedStation?.ocpp_version !== '1.6';
                  const unlockDisabled =
                    stationControlDisabled ||
                    connector.connector_id <= 0 ||
                    (requiresEvse && connector.evse_id == null);
                  return (
                    <div className="detail-card" key={`${connector.station_id}-${connector.connector_id}`}>
                      <div className="detail-line">
                        <span>Connettore</span>
                        <strong>{connector.connector_id}</strong>
                      </div>
                      <div className="detail-line">
                        <span>EVSE</span>
                        <strong>{connector.evse_id ?? 'n/a'}</strong>
                      </div>
                      <div className="detail-line">
                        <span>Stato</span>
                        <strong>{connector.current_status ?? 'n/a'}</strong>
                      </div>
                      <div className="detail-line">
                        <span>Errore</span>
                        <strong>{connector.current_error_code ?? 'n/a'}</strong>
                      </div>
                      <div className="detail-line">
                        <span>Attivo</span>
                        <strong>{connector.active ? 'Sì' : 'No'}</strong>
                      </div>
                      <div className="detail-line">
                        <span>Tx</span>
                        <strong>{connector.active_transaction_id ?? connector.active_transaction_ref ?? 'n/a'}</strong>
                      </div>
                      <div className="modal-actions">
                        <button
                          className="ghost-button"
                          type="button"
                          onClick={() => void toggleStationConnectorActive(selectedStation!.station_id, connector.connector_id, true)}
                          disabled={connectorActionDisabled}
                        >
                          Attiva
                        </button>
                        <button
                          className="ghost-button"
                          type="button"
                          onClick={() => void toggleStationConnectorActive(selectedStation!.station_id, connector.connector_id, false)}
                          disabled={connectorActionDisabled}
                        >
                          Disattiva
                        </button>
                        <button
                          className="ghost-button"
                          type="button"
                          onClick={() => void unlockStationConnector(selectedStation!.station_id, connector.connector_id)}
                          disabled={unlockDisabled}
                        >
                          Unlock
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>
            ) : (
              <div className="empty-state">Nessun connettore registrato.</div>
            )}
            <div className="modal-actions">
              <button
                className="ghost-button"
                type="button"
                onClick={() => void refreshStationStatus(selectedStation!.station_id)}
                disabled={stationControlDisabled}
              >
                Aggiorna stato
              </button>
              <button
                className="ghost-button"
                type="button"
                onClick={() => void toggleStationBlocked(selectedStation!.station_id, true)}
                disabled={stationControlDisabled}
              >
                Blocca colonnina
              </button>
              <button
                className="ghost-button"
                type="button"
                onClick={() => void toggleStationBlocked(selectedStation!.station_id, false)}
                disabled={stationControlDisabled}
              >
                Sblocca colonnina
              </button>
            </div>
            {formError ? <div className="empty-state error">{formError}</div> : null}
          </div>
        ) : (
          <form className="stack-form modal-form" onSubmit={handleSave}>
            {modalKind === 'create-user' ? (
              <>
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
              </>
            ) : null}

            {modalKind === 'edit-user' ? (
              <>
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
              </>
            ) : null}

            {modalKind === 'create-badge' ? (
              <>
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
                      <option value="">
                        Nessuno
                      </option>
                      {users.map((user) => (
                        <option key={user.id} value={user.id}>
                          {user.display_name}
                        </option>
                      ))}
                    </select>
                  </div>
                </label>
              </>
            ) : null}

            {modalKind === 'edit-badge' ? (
              <>
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
                      <option value="">
                        Nessuno
                      </option>
                      {users.map((user) => (
                        <option key={user.id} value={user.id}>
                          {user.display_name}
                        </option>
                      ))}
                    </select>
                  </div>
                </label>
              </>
            ) : null}

            {modalKind === 'station-location' ? (
              <>
                <div className="modal-readonly">
                  <div className="detail-title">
                    {selectedStation?.station_name?.trim() || selectedStation?.station_id || 'Nessuna colonnina selezionata'}
                  </div>
                  <p>Salvi solo i campi geografici e descrittivi. La colonnina arriva da OCPP.</p>
                </div>
                <label className="field">
                  <span>Nome colonnina</span>
                  <input
                    value={stationDraft.station_name}
                    onChange={(event) => setStationDraft((current) => ({ ...current, station_name: event.target.value }))}
                    placeholder="Colonnina parcheggio nord"
                  />
                </label>
                <div className="field-grid">
                  <label className="field">
                    <span>Latitude</span>
                    <input
                      value={stationDraft.latitude}
                      onChange={(event) => setStationDraft((current) => ({ ...current, latitude: event.target.value }))}
                      placeholder="46.2044"
                      inputMode="decimal"
                    />
                  </label>
                  <label className="field">
                    <span>Longitude</span>
                    <input
                      value={stationDraft.longitude}
                      onChange={(event) => setStationDraft((current) => ({ ...current, longitude: event.target.value }))}
                      placeholder="6.1432"
                      inputMode="decimal"
                    />
                  </label>
                </div>
                <label className="field">
                  <span>Etichetta posizione</span>
                  <input
                    value={stationDraft.location_label}
                    onChange={(event) => setStationDraft((current) => ({ ...current, location_label: event.target.value }))}
                    placeholder="Deposito Nord"
                  />
                </label>
                <label className="field">
                  <span>Indirizzo</span>
                  <input
                    value={stationDraft.address}
                    onChange={(event) => setStationDraft((current) => ({ ...current, address: event.target.value }))}
                    placeholder="Via esempio 12, Lugano"
                  />
                </label>
                <label className="field">
                  <span>Note</span>
                  <textarea
                    value={stationDraft.notes}
                    onChange={(event) => setStationDraft((current) => ({ ...current, notes: event.target.value }))}
                    rows={4}
                    placeholder="Accesso lato sud, vicino al cancello."
                  />
                </label>
              </>
            ) : null}

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
        )}
      </div>
    </div>
  );
}
