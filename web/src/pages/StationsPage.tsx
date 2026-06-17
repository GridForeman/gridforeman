import type { AppActions, AppData, StationStatus } from '../appTypes';

type Props = Pick<
  AppData,
  | 'stations'
  | 'selectedStation'
  | 'stationConnectors'
  | 'loadingStations'
  | 'loadingStationConnectors'
  | 'stationError'
  | 'stationConnectorsError'
  | 'onlineCount'
  | 'offlineCount'
  | 'errorCount'
  | 'locationCount'
  | 'lastBoot'
> & {
  actions: Pick<AppActions, 'openModal' | 'selectStation'>;
  timeAgo: (value: string) => string;
  formatStationName: (station: AppData['stations'][number]) => string;
  formatLocation: (station: AppData['stations'][number]) => string;
  getStationStatus: (station: AppData['stations'][number]) => StationStatus;
};

export function StationsPage({
  stations,
  selectedStation,
  stationConnectors,
  loadingStations,
  loadingStationConnectors,
  stationError,
  stationConnectorsError,
  onlineCount,
  offlineCount,
  errorCount,
  locationCount,
  lastBoot,
  actions,
  timeAgo,
  formatStationName,
  formatLocation,
  getStationStatus,
}: Props) {
  return (
    <>
      <section className="hero-grid">
        <article className="hero-card hero-card-large">
          <div className="hero-card-label">Stato rete</div>
          <div className="hero-card-value">{stations.length} colonnine registrate</div>
          <p>Vista operativa delle colonnine con presenza realtime, stato OCPP e mappa.</p>
          <div className="hero-metrics">
            <div>
              <span>Online</span>
              <strong>{onlineCount}</strong>
            </div>
            <div>
              <span>Offline</span>
              <strong>{offlineCount}</strong>
            </div>
            <div>
              <span>Errore</span>
              <strong>{errorCount}</strong>
            </div>
          </div>
        </article>

        <article className="hero-card">
          <div className="hero-card-label">Posizione</div>
          <div className="hero-card-value">{locationCount} già geolocalizzate</div>
          <p>Pronto per campo coordinate e visualizzazione su mappa.</p>
        </article>

        <article className="hero-card">
          <div className="hero-card-label">Ultimo boot</div>
          <div className="hero-card-value">{lastBoot ? timeAgo(lastBoot) : 'n/a'}</div>
          <p>Ultimo evento boot ricevuto da una colonnina.</p>
        </article>
      </section>

      <section className="content-grid">
        <article className="panel">
          <div className="panel-header">
            <div>
              <h2>Colonnine</h2>
              <p>Elenco iniziale con spazio per stato, posizione e dettaglio.</p>
            </div>
            <button className="ghost-button" type="button" onClick={() => actions.openModal('station-location')}>
              Modifica posizione
            </button>
          </div>

          {loadingStations ? (
            <div className="empty-state">Caricamento colonnine...</div>
          ) : stationError ? (
            <div className="empty-state error">{stationError}</div>
          ) : (
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>Station</th>
                    <th>Versione</th>
                    <th>Stato</th>
                    <th>Last seen</th>
                    <th>Peer</th>
                    <th>Boot</th>
                    <th>Posizione</th>
                  </tr>
                </thead>
                <tbody>
                  {stations.map((station) => {
                    const isSelected = station.station_id === selectedStation?.station_id;
                    const status = getStationStatus(station);
                    return (
                      <tr
                        key={station.station_id}
                        className={isSelected ? 'selected-row' : undefined}
                        onClick={() => actions.selectStation(station.station_id)}
                      >
                        <td>
                          <div className="station-id">{formatStationName(station)}</div>
                          <div className="station-id-muted">{station.station_id}</div>
                        </td>
                        <td>{station.ocpp_version}</td>
                        <td>
                          <span className={`pill ${station.blocked ? 'pill-error' : `pill-${status}`}`}>
                            {station.blocked ? 'bloccata' : status}
                          </span>
                        </td>
                        <td>{timeAgo(station.last_seen_at)}</td>
                        <td>{station.peer_addr}</td>
                        <td>{station.boot_count}</td>
                        <td>{formatLocation(station)}</td>
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
              <h2>Dettaglio</h2>
              <p>Pannello pronto per form, log e posizione geografica.</p>
            </div>
          </div>

          {selectedStation ? (
            <>
              <div className="detail-card">
                <div className="detail-title">{formatStationName(selectedStation)}</div>
                <div className="detail-subtitle">{selectedStation.station_id}</div>
                <div className="detail-line"><span>OCPP</span><strong>{selectedStation.ocpp_version}</strong></div>
                <div className="detail-line"><span>Peer</span><strong>{selectedStation.peer_addr}</strong></div>
                <div className="detail-line"><span>Stato OCPP</span><strong>{selectedStation.current_status ?? 'n/a'}</strong></div>
                <div className="detail-line"><span>Errore</span><strong>{selectedStation.current_error_code ?? 'n/a'}</strong></div>
                <div className="detail-line"><span>Blocco</span><strong>{selectedStation.blocked ? 'Bloccata' : 'Attiva'}</strong></div>
                <div className="detail-line"><span>Posizione</span><strong>{formatLocation(selectedStation)}</strong></div>
                <div className="detail-line"><span>Connettori</span><strong>{stationConnectors.filter((connector) => connector.station_id === selectedStation.station_id).length}</strong></div>
                <div className="detail-line">
                  <span>Lat/Lng</span>
                  <strong>
                    {selectedStation.latitude != null && selectedStation.longitude != null
                      ? `${selectedStation.latitude}, ${selectedStation.longitude}`
                      : 'n/a'}
                  </strong>
                </div>
                <button
                  className="primary-button"
                  type="button"
                  disabled={!selectedStation}
                  onClick={() => actions.openModal('station-location')}
                >
                  Modifica posizione
                </button>
                <button
                  className="ghost-button"
                  type="button"
                  disabled={!selectedStation}
                  onClick={() => actions.openModal('station-controls')}
                >
                  Gestione colonnina
                </button>
              </div>

              <div className="detail-card muted">
                <div className="detail-title">Connettori</div>
                {loadingStationConnectors ? (
                  <p>Carico connettori...</p>
                ) : stationConnectorsError ? (
                  <p>{stationConnectorsError}</p>
                ) : (
                  <p>{stationConnectors.filter((connector) => connector.station_id === selectedStation.station_id).length} connettori caricati.</p>
                )}
              </div>
            </>
          ) : (
            <div className="empty-state">Nessuna colonnina selezionata.</div>
          )}
        </aside>
      </section>
    </>
  );
}
