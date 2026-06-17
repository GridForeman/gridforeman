import { useEffect, useMemo, useState } from 'react';
import type { StationSummary } from '../api';
import { fetchEvents, type OcppEvent } from '../api';

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat('it-CH', {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(date);
}

function eventLabel(event: OcppEvent): string {
  const parts = [event.direction, event.action ?? event.parse_status];
  return parts.filter(Boolean).join(' · ');
}

function getEventStatus(event: OcppEvent): string {
  if (event.error) return 'Errore';

  if (event.action === 'Authorize' && event.direction === 'outbound') {
    try {
      const frame = JSON.parse(event.raw_text) as unknown;
      if (!Array.isArray(frame) || frame.length < 3) return 'OK';

      const payload = frame[2] as {
        id_tag_info?: { status?: string };
        id_token_info?: { status?: string };
      };

      const status = payload.id_tag_info?.status ?? payload.id_token_info?.status;
      return status ?? 'OK';
    } catch {
      return 'OK';
    }
  }

  return 'OK';
}

type Props = {
  stations: StationSummary[];
};

export function EventsPage({ stations }: Props) {
  const [events, setEvents] = useState<OcppEvent[]>([]);
  const [selectedEventId, setSelectedEventId] = useState<string | null>(null);
  const [stationFilter, setStationFilter] = useState<string>('');
  const [filterOpen, setFilterOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hasLoadedOnce, setHasLoadedOnce] = useState(false);

  const stationNameOptions = useMemo(() => {
    const options: Array<{ value: string; label: string }> = [{ value: '', label: 'Tutte le colonnine' }];
    const seen = new Set<string>();

    for (const station of stations) {
      const label = station.station_name?.trim();
      if (!label || seen.has(label)) continue;
      seen.add(label);
      options.push({ value: label, label });
    }

    if (stations.some((station) => !station.station_name)) {
      options.push({ value: '__unnamed__', label: 'Senza nome' });
    }

    return options;
  }, [stations]);

  async function loadEvents() {
    if (!hasLoadedOnce) {
      setLoading(true);
    } else {
      setRefreshing(true);
    }
    setError(null);
    try {
      const data = await fetchEvents(200, stationFilter || null);
      setEvents(data);
      setSelectedEventId((current) => current ?? data[0]?.message_id ?? null);
      setHasLoadedOnce(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Errore caricando gli eventi');
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }

  useEffect(() => {
    void loadEvents();
  }, [stationFilter]);

  const stationFilterLabel =
    stationFilter
      ? stationNameOptions.find((option) => option.value === stationFilter)?.label ?? stationFilter
      : 'Tutte le colonnine';

  const stationLabels = useMemo(() => {
    const labels = new Map<string, string>();
    for (const station of stations) {
      labels.set(station.station_id, station.station_name?.trim() || station.station_id);
    }
    return labels;
  }, [stations]);

  const selectedEvent = useMemo(
    () => events.find((event) => event.message_id === selectedEventId) ?? events[0] ?? null,
    [events, selectedEventId],
  );

  return (
    <section className="content-grid events-grid">
      <article className="panel panel-table">
        <div className="panel-header">
          <div>
            <h2>Eventi OCPP</h2>
            <p>Feed recente dei messaggi ricevuti dalle colonnine.</p>
          </div>
          <div className="topbar-actions events-toolbar">
            <div className="filter-menu">
              <button
                className="ghost-button filter-trigger"
                type="button"
                onClick={() => setFilterOpen((current) => !current)}
              >
                <span className="filter-trigger-label">{stationFilterLabel}</span>
                <span className="filter-trigger-caret">▾</span>
              </button>
              {filterOpen ? (
                <div className="filter-popover">
                  <button
                    className={`filter-option ${stationFilter === '' ? 'active' : ''}`}
                    type="button"
                    onClick={() => {
                      setStationFilter('');
                      setFilterOpen(false);
                    }}
                  >
                    Tutte le colonnine
                  </button>
                  {stationNameOptions
                    .filter((option) => option.value !== '')
                    .map((option) => (
                    <button
                      key={option.value}
                      className={`filter-option ${stationFilter === option.value ? 'active' : ''}`}
                      type="button"
                      onClick={() => {
                        setStationFilter(option.value);
                        setFilterOpen(false);
                      }}
                    >
                      {option.label}
                    </button>
                  ))}
                </div>
              ) : null}
            </div>
            <button className="ghost-button" type="button" disabled={loading || refreshing} onClick={() => void loadEvents()}>
              {refreshing ? 'Aggiorno...' : 'Aggiorna'}
            </button>
          </div>
        </div>

        {loading ? (
          <div className="empty-state">Caricamento eventi...</div>
        ) : error ? (
          <div className="empty-state error">{error}</div>
        ) : events.length === 0 ? (
          <div className="empty-state">Nessun evento salvato.</div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Quando</th>
                  <th>Colonnina</th>
                  <th>Tipo</th>
                  <th>Azione</th>
                  <th>Unique ID</th>
                  <th>Stato</th>
                </tr>
              </thead>
              <tbody>
                {events.map((event) => (
                  <tr
                    key={event.message_id}
                    className={event.message_id === selectedEvent?.message_id ? 'selected-row' : undefined}
                    onClick={() => setSelectedEventId(event.message_id)}
                  >
                    <td>{formatDate(event.received_at)}</td>
                    <td>
                      <div className="station-id">{stationLabels.get(event.station_id) ?? event.station_id}</div>
                    </td>
                    <td>{eventLabel(event)}</td>
                    <td>{event.action ?? 'n/a'}</td>
                    <td>{event.unique_id ?? 'n/a'}</td>
                    <td>
                      <span className={`pill ${event.error ? 'pill-error' : 'pill-online'}`}>
                        {getEventStatus(event)}
                      </span>
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
            <h2>Dettaglio evento</h2>
            <p>Payload grezzo e metadati della riga selezionata.</p>
          </div>
        </div>

        {selectedEvent ? (
          <>
            <div className="detail-card">
              <div className="detail-title">{selectedEvent.action ?? selectedEvent.parse_status}</div>
              <div className="detail-line"><span>Message ID</span><strong>{selectedEvent.message_id}</strong></div>
              <div className="detail-line"><span>Quando</span><strong>{formatDate(selectedEvent.received_at)}</strong></div>
              <div className="detail-line"><span>Station</span><strong>{stationLabels.get(selectedEvent.station_id) ?? selectedEvent.station_id}</strong></div>
              <div className="detail-line"><span>Station ID</span><strong>{selectedEvent.station_id}</strong></div>
              <div className="detail-line"><span>Versione</span><strong>{selectedEvent.ocpp_version}</strong></div>
              <div className="detail-line"><span>Peer</span><strong>{selectedEvent.peer_addr}</strong></div>
              <div className="detail-line"><span>Messaggio</span><strong>{selectedEvent.message_type ?? 'n/a'}</strong></div>
              <div className="detail-line"><span>Stato</span><strong>{selectedEvent.parse_status}</strong></div>
              <div className="detail-line"><span>Unique ID</span><strong>{selectedEvent.unique_id ?? 'n/a'}</strong></div>
              <div className="detail-line"><span>Errore</span><strong>{selectedEvent.error ?? 'n/a'}</strong></div>
            </div>

            <div className="detail-card muted">
              <div className="detail-title">Raw text</div>
              <pre className="code-block">{selectedEvent.raw_text}</pre>
            </div>

            <div className="detail-card muted">
              <div className="detail-title">Payload</div>
              <pre className="code-block">{selectedEvent.payload ?? 'n/a'}</pre>
            </div>
          </>
        ) : (
          <div className="empty-state">Nessun evento selezionato.</div>
        )}
      </aside>
    </section>
  );
}
