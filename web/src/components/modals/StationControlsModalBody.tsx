import type { StationControlsModalBodyProps } from './modalTypes';
import { isStationBlocked, stationAccessLabel } from '../../stations';

export function StationControlsModalBody({
  selectedStation,
  selectedStationConnectors,
  stationCommandBusy,
  formError,
  loadingStationConnectors,
  stationConnectorsError,
  closeModal,
  refreshStationStatus,
  toggleStationBlocked,
  toggleStationConnectorActive,
  unlockStationConnector,
}: StationControlsModalBodyProps) {
  const stationControlDisabled = stationCommandBusy || !selectedStation;
  const connectorActionDisabled = !selectedStation || loadingStationConnectors;
  const stationLabel = selectedStation?.station_name?.trim() || selectedStation?.station_id || 'Nessuna colonnina selezionata';
  const connectorCount = selectedStationConnectors.length;
  const stationBlocked = selectedStation ? isStationBlocked(selectedStation) : false;

  return (
    <div className="stack-form modal-form">
      <div className="modal-readonly">
        <div className="detail-title">{stationLabel}</div>
        <div className="detail-subtitle">{selectedStation?.station_id ?? 'n/a'}</div>
      </div>
      <div className="modal-stats-grid">
        <div className="modal-stat-card">
          <span>Versione</span>
          <strong>{selectedStation?.ocpp_version ?? 'n/a'}</strong>
        </div>
        <div className="modal-stat-card">
          <span>Accesso</span>
          <strong className={stationBlocked ? 'status-text-danger' : 'status-text-success'}>
            {selectedStation ? stationAccessLabel(selectedStation) : 'n/a'}
          </strong>
        </div>
        <div className="modal-stat-card">
          <span>Connettori</span>
          <strong>{connectorCount}</strong>
        </div>
      </div>
      <div className="modal-actions modal-actions-split">
        <button
          className="ghost-button"
          type="button"
          onClick={() => void refreshStationStatus(selectedStation!.station_id)}
          disabled={stationControlDisabled}
        >
          Aggiorna stato
        </button>
        <button
          className={stationBlocked ? 'ghost-button button-success' : 'ghost-button button-danger'}
          type="button"
          onClick={() => void toggleStationBlocked(selectedStation!.station_id, !stationBlocked)}
          disabled={stationControlDisabled}
        >
          {stationBlocked ? 'Sblocca colonnina' : 'Blocca colonnina'}
        </button>
      </div>

      {loadingStationConnectors ? (
        <div className="empty-state">Caricamento connettori...</div>
      ) : stationConnectorsError ? (
        <div className="empty-state error">{stationConnectorsError}</div>
      ) : selectedStationConnectors.length > 0 ? (
        <div className="connector-grid">
          {selectedStationConnectors.map((connector) => {
            const requiresEvse = selectedStation?.ocpp_version !== '1.6';
            const unlockDisabled =
              stationControlDisabled ||
              connector.connector_id <= 0 ||
              (requiresEvse && connector.evse_id == null);

            return (
              <div className="detail-card connector-card" key={`${connector.station_id}-${connector.connector_id}`}>
                <div className="connector-card-header">
                  <div>
                    <div className="detail-title">Connettore {connector.connector_id}</div>
                    <div className="detail-subtitle">EVSE {connector.evse_id ?? 'n/a'}</div>
                  </div>
                  <span className={`pill ${connector.active ? 'pill-online' : 'pill-error'}`}>
                    {connector.active ? 'Attivo' : 'Disattivo'}
                  </span>
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
        <button className="ghost-button" type="button" onClick={closeModal} disabled={stationCommandBusy}>
          Chiudi
        </button>
      </div>

      {formError ? <div className="empty-state error">{formError}</div> : null}
    </div>
  );
}
