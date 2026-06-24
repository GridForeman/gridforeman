import { useEffect, useMemo, useState } from 'react';
import type { StationControlsModalBodyProps } from './modalTypes';
import { isStationBlocked, stationAccessLabel } from '../../stations';

export function StationControlsModalBody({
  selectedStation,
  selectedStationConnectors,
  badges,
  stationConfiguration,
  stationCommandBusy,
  formError,
  loadingStationConnectors,
  stationConnectorsError,
  closeModal,
  refreshStationStatus,
  toggleStationBlocked,
  fetchStationConfiguration,
  remoteStartStationConnector,
  remoteStopStationConnector,
  setConnectorAutoRemoteStartBadge,
  toggleStationConnectorActive,
  unlockStationConnector,
}: StationControlsModalBodyProps) {
  const stationControlDisabled = stationCommandBusy || !selectedStation;
  const connectorActionDisabled = !selectedStation || loadingStationConnectors;
  const stationLabel = selectedStation?.station_name?.trim() || selectedStation?.station_id || 'Nessuna colonnina selezionata';
  const connectorCount = selectedStationConnectors.length;
  const stationBlocked = selectedStation ? isStationBlocked(selectedStation) : false;
  const remoteStartBadges = useMemo(
    () => badges.filter((badge) => badge.active && badge.user_id != null),
    [badges],
  );
  const [selectedBadgeCode, setSelectedBadgeCode] = useState('');
  const [autoStartBadgeCodes, setAutoStartBadgeCodes] = useState<Record<string, string>>({});

  useEffect(() => {
    if (!remoteStartBadges.some((badge) => badge.badge_code === selectedBadgeCode)) {
      setSelectedBadgeCode(remoteStartBadges[0]?.badge_code ?? '');
    }
  }, [remoteStartBadges, selectedBadgeCode]);

  useEffect(() => {
    setAutoStartBadgeCodes(
      Object.fromEntries(
        selectedStationConnectors.map((connector) => [
          `${connector.station_id}:${connector.connector_id}`,
          connector.auto_remote_start_badge_code ?? '',
        ]),
      ),
    );
  }, [selectedStationConnectors]);

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

      <div className="modal-actions">
        <button
          className="ghost-button"
          type="button"
          onClick={() => void fetchStationConfiguration(selectedStation!.station_id)}
          disabled={stationControlDisabled}
        >
          GetConfiguration
        </button>
      </div>

      {stationConfiguration ? (
        <div className="detail-card">
          <div className="connector-card-header">
            <div className="detail-title">Configurazione attuale</div>
            <span className="pill pill-online">{stationConfiguration.configuration_keys.length} chiavi</span>
          </div>
          {stationConfiguration.configuration_keys.length > 0 ? (
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>Chiave</th>
                    <th>Valore</th>
                    <th>Accesso</th>
                  </tr>
                </thead>
                <tbody>
                  {stationConfiguration.configuration_keys.map((entry) => (
                    <tr key={entry.key}>
                      <td>{entry.key}</td>
                      <td>{entry.value ?? 'n/a'}</td>
                      <td>{entry.readonly ? 'read-only' : 'scrivibile'}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="empty-state">Nessuna chiave restituita.</div>
          )}
          {stationConfiguration.unknown_keys.length > 0 ? (
            <div className="detail-line">
              <span>Chiavi sconosciute</span>
              <strong>{stationConfiguration.unknown_keys.join(', ')}</strong>
            </div>
          ) : null}
        </div>
      ) : null}

      <label className="field-label">
        Badge remote start
        <select
          value={selectedBadgeCode}
          onChange={(event) => setSelectedBadgeCode(event.target.value)}
          disabled={stationCommandBusy || remoteStartBadges.length === 0}
        >
          {remoteStartBadges.length === 0 ? (
            <option value="">Nessun badge attivo assegnato</option>
          ) : (
            remoteStartBadges.map((badge) => (
              <option key={badge.id} value={badge.badge_code}>
                {badge.label?.trim() || badge.badge_code}
              </option>
            ))
          )}
        </select>
      </label>

      {loadingStationConnectors ? (
        <div className="empty-state">Caricamento connettori...</div>
      ) : stationConnectorsError ? (
        <div className="empty-state error">{stationConnectorsError}</div>
      ) : selectedStationConnectors.length > 0 ? (
        <div className="connector-grid">
          {selectedStationConnectors.map((connector) => {
            const requiresEvse = selectedStation?.ocpp_version !== '1.6';
            const connectorKey = `${connector.station_id}:${connector.connector_id}`;
            const autoStartBadgeCode = autoStartBadgeCodes[connectorKey] ?? '';
            const unlockDisabled =
              stationControlDisabled ||
              connector.connector_id <= 0 ||
              (requiresEvse && connector.evse_id == null);
            const remoteStartDisabled =
              stationControlDisabled ||
              connector.connector_id <= 0 ||
              connector.active_transaction_id != null ||
              connector.active_transaction_ref != null ||
              !selectedBadgeCode;
            const remoteStopDisabled =
              stationControlDisabled ||
              connector.connector_id <= 0 ||
              (connector.active_transaction_id == null && connector.active_transaction_ref == null);

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
                <label className="field-label">
                  Badge auto avvio su Preparing
                  <select
                    value={autoStartBadgeCode}
                    onChange={(event) =>
                      setAutoStartBadgeCodes((current) => ({
                        ...current,
                        [connectorKey]: event.target.value,
                      }))
                    }
                    disabled={stationCommandBusy || remoteStartBadges.length === 0}
                  >
                    <option value="">Disabilitato</option>
                    {remoteStartBadges.map((badge) => (
                      <option key={badge.id} value={badge.badge_code}>
                        {badge.label?.trim() || badge.badge_code}
                      </option>
                    ))}
                  </select>
                </label>
                <div className="modal-actions">
                  <button
                    className="ghost-button button-success"
                    type="button"
                    onClick={() => void remoteStartStationConnector(selectedStation!.station_id, connector.connector_id, selectedBadgeCode)}
                    disabled={remoteStartDisabled}
                  >
                    Remote start
                  </button>
                  <button
                    className="ghost-button button-danger"
                    type="button"
                    onClick={() => void remoteStopStationConnector(selectedStation!.station_id, connector.connector_id)}
                    disabled={remoteStopDisabled}
                  >
                    Remote stop
                  </button>
                  <button
                    className="ghost-button"
                    type="button"
                    onClick={() =>
                      void setConnectorAutoRemoteStartBadge(
                        selectedStation!.station_id,
                        connector.connector_id,
                        autoStartBadgeCode || null,
                      )
                    }
                    disabled={stationCommandBusy || connector.connector_id <= 0}
                  >
                    Salva auto avvio
                  </button>
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
