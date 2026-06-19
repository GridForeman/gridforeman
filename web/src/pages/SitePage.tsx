import { useEffect, useMemo, useState } from 'react';
import {
  fetchSiteConfig,
  updateSiteConfig,
  type SiteConfigSnapshot,
  type SiteManagementGroup,
  type SitePowerFeed,
  type StationSummary,
} from '../api';

type Props = {
  stations: StationSummary[];
};

function newFeed(): SitePowerFeed {
  return {
    id: crypto.randomUUID(),
    name: '',
    meter_label: null,
    max_current_a: null,
    notes: null,
  };
}

function newGroup(): SiteManagementGroup {
  return {
    id: crypto.randomUUID(),
    name: '',
    control_mode: 'load-sharing',
    power_feed_ids: [],
    station_ids: [],
    notes: null,
  };
}

function defaultConfig(): SiteConfigSnapshot {
  return {
    site_name: null,
    timezone: 'Europe/Zurich',
    operator_name: null,
    notes: null,
    power_feeds: [],
    management_groups: [],
    updated_at: null,
  };
}

function parseNumberOrNull(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : null;
}

function normalizeInput(value: string): string | null {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function formatDateTime(value: string | null): string {
  if (!value) return 'mai';
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return value;
  return new Intl.DateTimeFormat('it-CH', {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(parsed);
}

export function SitePage({ stations }: Props) {
  const [config, setConfig] = useState<SiteConfigSnapshot>(defaultConfig);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void fetchSiteConfig()
      .then((snapshot) => {
        if (!active) return;
        setConfig(snapshot);
        setError(null);
      })
      .catch((err) => {
        if (!active) return;
        setError(err instanceof Error ? err.message : 'Errore caricando configurazione impianto');
      })
      .finally(() => {
        if (active) {
          setLoading(false);
        }
      });

    return () => {
      active = false;
    };
  }, []);

  const assignedStationCount = useMemo(
    () => new Set(config.management_groups.flatMap((group) => group.station_ids)).size,
    [config.management_groups],
  );

  function updateFeed(feedId: string, patch: Partial<SitePowerFeed>) {
    setConfig((current) => ({
      ...current,
      power_feeds: current.power_feeds.map((feed) => (feed.id === feedId ? { ...feed, ...patch } : feed)),
    }));
  }

  function updateGroup(groupId: string, patch: Partial<SiteManagementGroup>) {
    setConfig((current) => ({
      ...current,
      management_groups: current.management_groups.map((group) => (group.id === groupId ? { ...group, ...patch } : group)),
    }));
  }

  function toggleGroupFeed(groupId: string, feedId: string) {
    setConfig((current) => ({
      ...current,
      management_groups: current.management_groups.map((group) => {
        if (group.id !== groupId) return group;
        const exists = group.power_feed_ids.includes(feedId);
        return {
          ...group,
          power_feed_ids: exists
            ? group.power_feed_ids.filter((value) => value !== feedId)
            : [...group.power_feed_ids, feedId],
        };
      }),
    }));
  }

  function toggleGroupStation(groupId: string, stationId: string) {
    setConfig((current) => ({
      ...current,
      management_groups: current.management_groups.map((group) => {
        if (group.id !== groupId) return group;
        const exists = group.station_ids.includes(stationId);
        return {
          ...group,
          station_ids: exists
            ? group.station_ids.filter((value) => value !== stationId)
            : [...group.station_ids, stationId],
        };
      }),
    }));
  }

  async function handleSave() {
    setSaving(true);
    setError(null);
    try {
      const saved = await updateSiteConfig({
        ...config,
        site_name: normalizeInput(config.site_name ?? ''),
        timezone: config.timezone,
        operator_name: normalizeInput(config.operator_name ?? ''),
        notes: normalizeInput(config.notes ?? ''),
        power_feeds: config.power_feeds.map((feed) => ({
          ...feed,
          name: feed.name,
          meter_label: normalizeInput(feed.meter_label ?? ''),
          notes: normalizeInput(feed.notes ?? ''),
        })),
        management_groups: config.management_groups.map((group) => ({
          ...group,
          name: group.name,
          control_mode: group.control_mode,
          notes: normalizeInput(group.notes ?? ''),
        })),
      });
      setConfig(saved);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Errore salvando configurazione impianto');
    } finally {
      setSaving(false);
    }
  }

  return (
    <>
      <section className="hero-grid">
        <article className="hero-card hero-card-large">
          <div className="hero-card-label">Impianto</div>
          <div className="hero-card-value">{config.site_name?.trim() || 'Configurazione locale'}</div>
          <p>Configura topologia locale senza assumere un solo gruppo di gestione per impianto.</p>
          <div className="hero-metrics">
            <div>
              <span>Feed potenza</span>
              <strong>{config.power_feeds.length}</strong>
            </div>
            <div>
              <span>Gruppi gestione</span>
              <strong>{config.management_groups.length}</strong>
            </div>
            <div>
              <span>Colonnine assegnate</span>
              <strong>{assignedStationCount}</strong>
            </div>
          </div>
        </article>

        <article className="hero-card">
          <div className="hero-card-label">Stazioni note</div>
          <div className="hero-card-value">{stations.length}</div>
          <p>Colonnine disponibili per associazione ai gruppi di gestione.</p>
        </article>

        <article className="hero-card">
          <div className="hero-card-label">Ultimo salvataggio</div>
          <div className="hero-card-value">{formatDateTime(config.updated_at)}</div>
          <p>Snapshot attuale della configurazione locale dell&apos;impianto.</p>
        </article>
      </section>

      <section className="content-grid site-config-grid">
        <article className="panel">
          <div className="panel-header">
            <div>
              <h2>Identità impianto</h2>
              <p>Metadati generali e note operative locali.</p>
            </div>
            <button className="primary-button" type="button" onClick={handleSave} disabled={loading || saving}>
              {saving ? 'Salvataggio...' : 'Salva configurazione'}
            </button>
          </div>

          {loading ? (
            <div className="empty-state">Caricamento configurazione impianto...</div>
          ) : (
            <div className="stack-form">
              {error ? <div className="empty-state error">{error}</div> : null}
              <div className="field-grid">
                <label className="field">
                  <span>Nome impianto</span>
                  <input
                    value={config.site_name ?? ''}
                    onChange={(event) => setConfig((current) => ({ ...current, site_name: event.target.value }))}
                    placeholder="Sede Rivera, Parcheggio nord..."
                  />
                </label>
                <label className="field">
                  <span>Timezone</span>
                  <input
                    value={config.timezone}
                    onChange={(event) => setConfig((current) => ({ ...current, timezone: event.target.value }))}
                    placeholder="Europe/Zurich"
                  />
                </label>
              </div>

              <div className="field-grid">
                <label className="field">
                  <span>Operatore</span>
                  <input
                    value={config.operator_name ?? ''}
                    onChange={(event) => setConfig((current) => ({ ...current, operator_name: event.target.value }))}
                    placeholder="Azienda / installatore"
                  />
                </label>
                <div className="field">
                  <span>Modello</span>
                  <input value="Feed multipli + gruppi multipli" readOnly />
                </div>
              </div>

              <label className="field">
                <span>Note</span>
                <textarea
                  rows={4}
                  value={config.notes ?? ''}
                  onChange={(event) => setConfig((current) => ({ ...current, notes: event.target.value }))}
                  placeholder="Vincoli impianto, logica di bilanciamento, informazioni di campo..."
                />
              </label>
            </div>
          )}
        </article>

        <aside className="panel panel-side">
          <div className="panel-header">
            <div>
              <h2>Feed di potenza</h2>
              <p>Entrate aziendali, linee o sorgenti da cui derivare limiti disponibili.</p>
            </div>
            <button
              className="ghost-button"
              type="button"
              onClick={() => setConfig((current) => ({ ...current, power_feeds: [...current.power_feeds, newFeed()] }))}
              disabled={loading || saving}
            >
              Nuovo feed
            </button>
          </div>

          {config.power_feeds.length === 0 ? (
            <div className="empty-state">Nessun feed definito.</div>
          ) : (
            config.power_feeds.map((feed, index) => (
              <div key={feed.id} className="detail-card site-config-card">
                <div className="panel-header">
                  <div>
                    <div className="detail-title">Feed {index + 1}</div>
                    <div className="detail-subtitle">{feed.id}</div>
                  </div>
                  <button
                    className="ghost-button small-button button-danger"
                    type="button"
                    onClick={() => setConfig((current) => ({
                      ...current,
                      power_feeds: current.power_feeds.filter((candidate) => candidate.id !== feed.id),
                      management_groups: current.management_groups.map((group) => ({
                        ...group,
                        power_feed_ids: group.power_feed_ids.filter((value) => value !== feed.id),
                      })),
                    }))}
                    disabled={saving}
                  >
                    Rimuovi
                  </button>
                </div>

                <div className="stack-form">
                  <div className="field-grid">
                    <label className="field">
                      <span>Codice feed</span>
                      <input value={feed.id} onChange={(event) => updateFeed(feed.id, { id: event.target.value })} />
                    </label>
                    <label className="field">
                      <span>Nome</span>
                      <input value={feed.name} onChange={(event) => updateFeed(feed.id, { name: event.target.value })} />
                    </label>
                  </div>
                  <div className="field-grid">
                    <label className="field">
                      <span>Meter label</span>
                      <input
                        value={feed.meter_label ?? ''}
                        onChange={(event) => updateFeed(feed.id, { meter_label: event.target.value })}
                        placeholder="contatore_qg_nord"
                      />
                    </label>
                    <label className="field">
                      <span>Corrente max A</span>
                      <input
                        value={feed.max_current_a ?? ''}
                        onChange={(event) => updateFeed(feed.id, { max_current_a: parseNumberOrNull(event.target.value) })}
                        placeholder="250"
                      />
                    </label>
                  </div>
                  <label className="field">
                    <span>Note</span>
                    <textarea
                      rows={3}
                      value={feed.notes ?? ''}
                      onChange={(event) => updateFeed(feed.id, { notes: event.target.value })}
                    />
                  </label>
                </div>
              </div>
            ))
          )}
        </aside>
      </section>

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2>Gruppi di gestione</h2>
            <p>Ogni gruppo può usare più feed e governare più colonnine. Nessun vincolo a gruppo unico per impianto.</p>
          </div>
          <button
            className="ghost-button"
            type="button"
            onClick={() => setConfig((current) => ({
              ...current,
              management_groups: [...current.management_groups, newGroup()],
            }))}
            disabled={loading || saving}
          >
            Nuovo gruppo
          </button>
        </div>

        {config.management_groups.length === 0 ? (
          <div className="empty-state">Nessun gruppo di gestione definito.</div>
        ) : (
          <div className="site-group-grid">
            {config.management_groups.map((group, index) => (
              <div key={group.id} className="detail-card site-config-card">
                <div className="panel-header">
                  <div>
                    <div className="detail-title">Gruppo {index + 1}</div>
                    <div className="detail-subtitle">{group.id}</div>
                  </div>
                  <button
                    className="ghost-button small-button button-danger"
                    type="button"
                    onClick={() => setConfig((current) => ({
                      ...current,
                      management_groups: current.management_groups.filter((candidate) => candidate.id !== group.id),
                    }))}
                    disabled={saving}
                  >
                    Rimuovi
                  </button>
                </div>

                <div className="stack-form">
                  <div className="field-grid">
                    <label className="field">
                      <span>Codice gruppo</span>
                      <input value={group.id} onChange={(event) => updateGroup(group.id, { id: event.target.value })} />
                    </label>
                    <label className="field">
                      <span>Nome</span>
                      <input value={group.name} onChange={(event) => updateGroup(group.id, { name: event.target.value })} />
                    </label>
                  </div>

                  <label className="field">
                    <span>Modalità controllo</span>
                    <div className="field-select-shell">
                      <select
                        className="field-select"
                        value={group.control_mode}
                        onChange={(event) => updateGroup(group.id, { control_mode: event.target.value })}
                      >
                        <option value="load-sharing">load-sharing</option>
                        <option value="priority-based">priority-based</option>
                        <option value="static-limit">static-limit</option>
                      </select>
                    </div>
                  </label>

                  <div className="site-config-checklist">
                    <div className="site-config-checklist-title">Feed associati</div>
                    {config.power_feeds.length === 0 ? (
                      <div className="empty-state">Prima definisci almeno un feed.</div>
                    ) : (
                      config.power_feeds.map((feed) => (
                        <label key={feed.id} className="site-config-check">
                          <input
                            type="checkbox"
                            checked={group.power_feed_ids.includes(feed.id)}
                            onChange={() => toggleGroupFeed(group.id, feed.id)}
                          />
                          <span>{feed.name.trim() || feed.id}</span>
                        </label>
                      ))
                    )}
                  </div>

                  <div className="site-config-checklist">
                    <div className="site-config-checklist-title">Colonnine assegnate</div>
                    {stations.length === 0 ? (
                      <div className="empty-state">Nessuna colonnina registrata.</div>
                    ) : (
                      stations.map((station) => (
                        <label key={station.station_id} className="site-config-check">
                          <input
                            type="checkbox"
                            checked={group.station_ids.includes(station.station_id)}
                            onChange={() => toggleGroupStation(group.id, station.station_id)}
                          />
                          <span>{station.station_name?.trim() || station.station_id}</span>
                        </label>
                      ))
                    )}
                  </div>

                  <label className="field">
                    <span>Note</span>
                    <textarea
                      rows={3}
                      value={group.notes ?? ''}
                      onChange={(event) => updateGroup(group.id, { notes: event.target.value })}
                    />
                  </label>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>
    </>
  );
}
