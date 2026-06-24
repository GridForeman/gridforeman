import { useEffect, useMemo, useState } from 'react';
import { fetchTransactions, type Badge, type ChargingTransaction, type StationSummary, type User } from '../api';

function formatDate(value: string | null): string {
  if (!value) return 'n/a';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat('it-CH', {
    dateStyle: 'medium',
    timeStyle: 'medium',
  }).format(date);
}

function formatEnergy(energyWh: number | null): string {
  if (energyWh == null) return 'n/a';
  return `${(energyWh / 1000).toFixed(2)} kWh`;
}

function formatDuration(startedAt: string, endedAt: string | null): string {
  const start = new Date(startedAt).getTime();
  const end = endedAt ? new Date(endedAt).getTime() : Date.now();
  if (Number.isNaN(start) || Number.isNaN(end) || end < start) return 'n/a';

  const totalSeconds = Math.round((end - start) / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${hours} h ${minutes} min ${seconds} s`;
  if (minutes > 0) return `${minutes} min ${seconds} s`;
  return `${seconds} s`;
}

function statusLabel(status: string): string {
  if (status === 'in_progress') return 'In corso';
  if (status === 'completed') return 'Completata';
  if (status === 'invalid') return 'RFID non valido';
  if (status === 'blocked') return 'Badge bloccato';
  return status;
}

function statusClassName(status: string): string {
  if (status === 'in_progress') return 'pill-idle';
  if (status === 'completed') return 'pill-online';
  if (status === 'invalid' || status === 'blocked') return 'pill-error';
  return 'pill-error';
}

type Props = {
  stations: StationSummary[];
  users: User[];
  badges: Badge[];
};

export function TransactionsPage({ stations, users, badges }: Props) {
  const [transactions, setTransactions] = useState<ChargingTransaction[]>([]);
  const [selectedTransactionId, setSelectedTransactionId] = useState<number | null>(null);
  const [userFilter, setUserFilter] = useState<string>('');
  const [badgeFilter, setBadgeFilter] = useState<string>('');
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const stationLabels = useMemo(() => {
    const labels = new Map<string, string>();
    for (const station of stations) {
      labels.set(station.station_id, station.station_name?.trim() || station.station_id);
    }
    return labels;
  }, [stations]);

  const userLabels = useMemo(() => {
    const labels = new Map<number, string>();
    for (const user of users) {
      labels.set(user.id, user.display_name);
    }
    return labels;
  }, [users]);

  const badgeLabels = useMemo(() => {
    const labels = new Map<number, string>();
    for (const badge of badges) {
      labels.set(badge.id, badge.label?.trim() || badge.badge_code);
    }
    return labels;
  }, [badges]);

  const userOptions = useMemo(
    () => users.map((user) => ({ value: String(user.id), label: user.display_name })),
    [users],
  );

  const badgeOptions = useMemo(() => {
    const filteredBadges = userFilter
      ? badges.filter((badge) => badge.user_id === Number(userFilter))
      : badges;

    return filteredBadges.map((badge) => ({
      value: String(badge.id),
      label: badge.label?.trim() || badge.badge_code,
    }));
  }, [badges, userFilter]);

  async function loadTransactions(keepVisible = true) {
    if (refreshing || (keepVisible && loading)) return;
    if (!keepVisible) {
      setLoading(true);
    } else {
      setRefreshing(true);
    }
    setError(null);
    try {
      const data = await fetchTransactions(200);
      setTransactions(data);
      setSelectedTransactionId((current) => current ?? data[0]?.id ?? null);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Errore caricando le transazioni');
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }

  useEffect(() => {
    void loadTransactions(false);
  }, []);

  useEffect(() => {
    const intervalId = window.setInterval(() => {
      void loadTransactions();
    }, 5000);

    return () => {
      window.clearInterval(intervalId);
    };
  }, []);

  useEffect(() => {
    if (!badgeFilter) return;
    if (badgeOptions.some((badge) => badge.value === badgeFilter)) return;
    setBadgeFilter('');
  }, [badgeFilter, badgeOptions]);

  const filteredTransactions = useMemo(
    () =>
      transactions.filter((transaction) => {
        if (userFilter && String(transaction.user_id ?? '') !== userFilter) return false;
        if (badgeFilter && String(transaction.badge_id ?? '') !== badgeFilter) return false;
        return true;
      }),
    [transactions, userFilter, badgeFilter],
  );

  useEffect(() => {
    if (filteredTransactions.some((transaction) => transaction.id === selectedTransactionId)) return;
    setSelectedTransactionId(filteredTransactions[0]?.id ?? null);
  }, [filteredTransactions, selectedTransactionId]);

  const selectedTransaction = useMemo(
    () =>
      filteredTransactions.find((transaction) => transaction.id === selectedTransactionId)
      ?? filteredTransactions[0]
      ?? null,
    [filteredTransactions, selectedTransactionId],
  );

  const inProgressCount = filteredTransactions.filter((transaction) => transaction.status === 'in_progress').length;
  const completedCount = filteredTransactions.filter((transaction) => transaction.status === 'completed').length;
  const totalEnergyWh = filteredTransactions.reduce((sum, transaction) => sum + (transaction.energy_wh ?? 0), 0);

  return (
    <section className="content-grid transactions-grid">
      <article className="panel panel-table">
        <div className="panel-header">
          <div className="topbar-actions transactions-toolbar">
            <label className="transactions-filter">
              <span className="transactions-filter-label">Utente</span>
              <span className="field-select-shell transactions-filter-shell">
                <select
                  className="field-select transactions-filter-select"
                  value={userFilter}
                  onChange={(event) => setUserFilter(event.target.value)}
                >
                  <option value="">Tutti gli utenti</option>
                  {userOptions.map((user) => (
                    <option key={user.value} value={user.value}>
                      {user.label}
                    </option>
                  ))}
                </select>
              </span>
            </label>
            <label className="transactions-filter">
              <span className="transactions-filter-label">Badge</span>
              <span className="field-select-shell transactions-filter-shell">
                <select
                  className="field-select transactions-filter-select"
                  value={badgeFilter}
                  onChange={(event) => setBadgeFilter(event.target.value)}
                >
                  <option value="">Tutti i badge</option>
                  {badgeOptions.map((badge) => (
                    <option key={badge.value} value={badge.value}>
                      {badge.label}
                    </option>
                  ))}
                </select>
              </span>
            </label>
            <button
              className="ghost-button toolbar-refresh-button"
              type="button"
              onClick={() => void loadTransactions(true)}
            >
              Aggiorna
            </button>
          </div>
        </div>

        <div className="hero-metrics transactions-metrics">
          <div>
            <span>In corso</span>
            <strong>{inProgressCount}</strong>
          </div>
          <div>
            <span>Completate</span>
            <strong>{completedCount}</strong>
          </div>
          <div>
            <span>Energia</span>
            <strong>{formatEnergy(totalEnergyWh)}</strong>
          </div>
        </div>

        {loading ? (
          <div className="empty-state">Caricamento transazioni...</div>
        ) : error ? (
          <div className="empty-state error">{error}</div>
        ) : filteredTransactions.length === 0 ? (
          <div className="empty-state">Nessuna transazione salvata.</div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>Inizio</th>
                  <th>Colonnina</th>
                  <th>Utente</th>
                  <th>Badge</th>
                  <th>Stato</th>
                  <th>Durata</th>
                  <th>Energia</th>
                </tr>
              </thead>
              <tbody>
                {filteredTransactions.map((transaction) => (
                  <tr
                    key={transaction.id}
                    className={transaction.id === selectedTransaction?.id ? 'selected-row' : undefined}
                    onClick={() => setSelectedTransactionId(transaction.id)}
                  >
                    <td>{formatDate(transaction.started_at)}</td>
                    <td>
                      <div className="station-id">{stationLabels.get(transaction.station_id) ?? transaction.station_id}</div>
                    </td>
                    <td>{transaction.user_id != null ? userLabels.get(transaction.user_id) ?? `#${transaction.user_id}` : 'n/a'}</td>
                    <td>{transaction.badge_id != null ? badgeLabels.get(transaction.badge_id) ?? transaction.badge_code ?? `#${transaction.badge_id}` : transaction.badge_code ?? 'n/a'}</td>
                    <td>
                      <span className={`pill ${statusClassName(transaction.status)}`}>
                        {statusLabel(transaction.status)}
                      </span>
                    </td>
                    <td>{formatDuration(transaction.started_at, transaction.ended_at)}</td>
                    <td>{formatEnergy(transaction.energy_wh)}</td>
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
            <h2>Dettaglio</h2>
            <p>Identificativi OCPP, misure, esito e collegamenti utente/badge.</p>
          </div>
        </div>

        {selectedTransaction ? (
          <>
            <div className="detail-card">
              <div className="detail-title">{stationLabels.get(selectedTransaction.station_id) ?? selectedTransaction.station_id}</div>
              <div className="detail-subtitle">Tx #{selectedTransaction.id}</div>
              <div className="detail-line"><span>Stato</span><strong>{statusLabel(selectedTransaction.status)}</strong></div>
              <div className="detail-line"><span>Utente</span><strong>{selectedTransaction.user_id != null ? userLabels.get(selectedTransaction.user_id) ?? `#${selectedTransaction.user_id}` : 'n/a'}</strong></div>
              <div className="detail-line"><span>Badge</span><strong>{selectedTransaction.badge_code ?? 'n/a'}</strong></div>
              <div className="detail-line"><span>Inizio</span><strong>{formatDate(selectedTransaction.started_at)}</strong></div>
              <div className="detail-line"><span>Fine</span><strong>{formatDate(selectedTransaction.ended_at)}</strong></div>
              <div className="detail-line"><span>Durata</span><strong>{formatDuration(selectedTransaction.started_at, selectedTransaction.ended_at)}</strong></div>
              <div className="detail-line"><span>Energia</span><strong>{formatEnergy(selectedTransaction.energy_wh)}</strong></div>
              <div className="detail-line"><span>Meter start</span><strong>{selectedTransaction.meter_start_wh ?? 'n/a'} Wh</strong></div>
              <div className="detail-line"><span>Meter stop</span><strong>{selectedTransaction.meter_stop_wh ?? 'n/a'} Wh</strong></div>
              <div className="detail-line"><span>Ultimo meter</span><strong>{selectedTransaction.last_meter_wh ?? 'n/a'} Wh</strong></div>
            </div>

            <div className="detail-card muted">
              <div className="detail-title">OCPP</div>
              <div className="detail-line"><span>Versione</span><strong>{selectedTransaction.ocpp_version}</strong></div>
              <div className="detail-line"><span>Station ID</span><strong>{selectedTransaction.station_id}</strong></div>
              <div className="detail-line"><span>Connector</span><strong>{selectedTransaction.connector_id ?? 'n/a'}</strong></div>
              <div className="detail-line"><span>EVSE</span><strong>{selectedTransaction.evse_id ?? 'n/a'}</strong></div>
              <div className="detail-line"><span>Tx ID</span><strong>{selectedTransaction.ocpp_transaction_id ?? 'n/a'}</strong></div>
              <div className="detail-line"><span>Tx Ref</span><strong>{selectedTransaction.ocpp_transaction_ref ?? 'n/a'}</strong></div>
              <div className="detail-line"><span>Stop reason</span><strong>{selectedTransaction.stop_reason ?? 'n/a'}</strong></div>
            </div>
          </>
        ) : (
          <div className="empty-state">Nessuna transazione selezionata.</div>
        )}
      </aside>
    </section>
  );
}
