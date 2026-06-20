import { useEffect, useMemo, useState } from 'react';
import {
  fetchSiteConfig,
  updateSiteConfig,
  type SiteConfigSnapshot,
  type SiteManagementGroup,
  type StationSummary,
} from '../api';

type Props = {
  stations: StationSummary[];
};

function newGroup(): SiteManagementGroup {
  return {
    id:
      globalThis.crypto && typeof globalThis.crypto.randomUUID === 'function'
        ? globalThis.crypto.randomUUID()
        : `group-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`,
    name: '',
    control_mode: 'load-sharing',
    energy_meter_ids: [],
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
    energy_meters: [],
    management_groups: [],
    updated_at: null,
  };
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

  function updateGroup(groupId: string, patch: Partial<SiteManagementGroup>) {
    setConfig((current) => ({
      ...current,
      management_groups: current.management_groups.map((group) => (group.id === groupId ? { ...group, ...patch } : group)),
    }));
  }

  function toggleGroupEnergyMeter(groupId: string, meterId: string) {
    setConfig((current) => ({
      ...current,
      management_groups: current.management_groups.map((group) => {
        if (group.id !== groupId) return group;
        const exists = group.energy_meter_ids.includes(meterId);
        return {
          ...group,
          energy_meter_ids: exists
            ? group.energy_meter_ids.filter((value) => value !== meterId)
            : [...group.energy_meter_ids, meterId],
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
              <span>Misuratori energia</span>
              <strong>{config.energy_meters.length}</strong>
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
                  <input value="Misuratori energia multipli + gruppi multipli" readOnly />
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

      </section>

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2>Gruppi di gestione</h2>
            <p>Ogni gruppo può usare più misuratori energia e governare più colonnine. Nessun vincolo a gruppo unico per impianto.</p>
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
                    <div className="site-config-checklist-title">Misuratori energia associati</div>
                    {config.energy_meters.length === 0 ? (
                      <div className="empty-state">Prima definisci almeno un misuratore energia.</div>
                    ) : (
                      config.energy_meters.map((meter) => (
                        <label key={meter.id} className="site-config-check">
                          <input
                            type="checkbox"
                            checked={group.energy_meter_ids.includes(meter.id)}
                            onChange={() => toggleGroupEnergyMeter(group.id, meter.id)}
                          />
                          <span>{meter.name.trim() || meter.id}</span>
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
