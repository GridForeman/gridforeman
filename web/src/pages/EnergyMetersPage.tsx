import { useEffect, useState } from 'react';
import { ModalShell } from '../components/modals/ModalShell';
import {
  createEnergyMeter,
  deleteEnergyMeter,
  fetchEnergyMeterCatalog,
  type EnergyMeterCatalog,
  type EnergyMeter,
  type SiteEnergyMeter,
  fetchEnergyMeters,
  updateEnergyMeterRecord,
} from '../api';

function newLocalId(prefix: string): string {
  if (globalThis.crypto && typeof globalThis.crypto.randomUUID === 'function') {
    return globalThis.crypto.randomUUID();
  }

  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

function newEnergyMeter(): SiteEnergyMeter {
  return {
    id: newLocalId('meter'),
    name: '',
    catalog_key: null,
    host: null,
    port: 502,
    unit_id: 1,
    poll_interval_ms: 1000,
    meter_label: null,
    max_current_a: null,
    notes: null,
  };
}

function parseNumberOrNull(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : null;
}

function parseIntegerOrNull(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number.parseInt(trimmed, 10);
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

export function EnergyMetersPage() {
  const [meters, setMeters] = useState<EnergyMeter[]>([]);
  const [catalog, setCatalog] = useState<EnergyMeterCatalog>({ profiles: [] });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [meterModalMode, setMeterModalMode] = useState<'create' | 'edit' | null>(null);
  const [selectedMeterId, setSelectedMeterId] = useState<string | null>(null);
  const [meterDraft, setMeterDraft] = useState<SiteEnergyMeter>(newEnergyMeter);

  useEffect(() => {
    let active = true;
    void Promise.all([fetchEnergyMeters(), fetchEnergyMeterCatalog()])
      .then(([loadedMeters, loadedCatalog]) => {
        if (!active) return;
        setMeters(loadedMeters);
        setCatalog(loadedCatalog);
        setError(null);
      })
      .catch((err) => {
        if (!active) return;
        setError(err instanceof Error ? err.message : 'Errore caricando misuratori energia');
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

  function updateMeterDraft(patch: Partial<SiteEnergyMeter>) {
    setMeterDraft((current) => ({ ...current, ...patch }));
  }

  function openCreateMeterModal() {
    setMeterDraft(newEnergyMeter());
    setMeterModalMode('create');
  }

  function openEditMeterModal(meter: SiteEnergyMeter) {
    setSelectedMeterId(meter.id);
    setMeterDraft({ ...meter });
    setMeterModalMode('edit');
  }

  function closeMeterModal() {
    if (saving) return;
    setMeterModalMode(null);
    setSelectedMeterId(null);
  }

  function handleSubmitMeterModal() {
    setSaving(true);
    setError(null);
    const payload = {
      ...meterDraft,
      name: meterDraft.name,
      catalog_key: normalizeInput(meterDraft.catalog_key ?? ''),
      host: normalizeInput(meterDraft.host ?? ''),
      meter_label: normalizeInput(meterDraft.meter_label ?? ''),
      notes: normalizeInput(meterDraft.notes ?? ''),
    };
    const currentMode = meterModalMode;
    const currentSelectedMeterId = selectedMeterId;
    void (async () => {
      try {
        if (currentMode === 'edit' && currentSelectedMeterId) {
          const updated = await updateEnergyMeterRecord(currentSelectedMeterId, payload);
          setMeters((current) => current.map((meter) => (meter.id === currentSelectedMeterId ? updated : meter)));
        } else {
          const created = await createEnergyMeter(payload);
          setMeters((current) => [...current, created]);
        }
        setMeterModalMode(null);
        setSelectedMeterId(null);
        setMeterDraft(newEnergyMeter());
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Errore salvando misuratore energia');
      } finally {
        setSaving(false);
      }
    })();
  }

  async function handleDeleteMeter(meterId: string) {
    setSaving(true);
    setError(null);
    try {
      await deleteEnergyMeter(meterId);
      setMeters((current) => current.filter((meter) => meter.id !== meterId));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Errore rimuovendo misuratore energia');
    } finally {
      setSaving(false);
    }
  }

  const meterCount = meters.length;
  const lastUpdatedAt = meters.reduce<string | null>((latest, meter) => {
    if (!latest) return meter.updated_at;
    return new Date(meter.updated_at).getTime() > new Date(latest).getTime() ? meter.updated_at : latest;
  }, null);

  return (
    <>
      <section className="hero-grid">
        <article className="hero-card hero-card-large">
          <div className="hero-card-label">Misuratori</div>
          <div className="hero-card-value">{meterCount}</div>
          <p>Catalogo statico profili Modbus TCP/IP piu configurazione installata per impianto.</p>
          <div className="hero-metrics">
            <div>
              <span>Profili libreria</span>
              <strong>{catalog.profiles.length}</strong>
            </div>
            <div>
              <span>Misuratori configurati</span>
              <strong>{meterCount}</strong>
            </div>
            <div>
              <span>Ultimo salvataggio</span>
              <strong>{formatDateTime(lastUpdatedAt)}</strong>
            </div>
          </div>
        </article>
      </section>

      <article className="panel panel-table">
        <div className="panel-header">
          <div>
            <h2>Misuratori energia</h2>
            <p>Definisci profilo libreria, endpoint Modbus e parametri polling per ogni misuratore installato.</p>
          </div>
          <button
            className="primary-button"
            type="button"
            onClick={openCreateMeterModal}
            disabled={loading || saving}
          >
            Nuovo misuratore
          </button>
        </div>

        {loading ? (
          <div className="empty-state">Caricamento misuratori energia...</div>
        ) : error ? (
          <div className="empty-state error">{error}</div>
        ) : meters.length === 0 ? (
          <div className="empty-state">Nessun misuratore energia definito.</div>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>ID</th>
                  <th>Nome</th>
                  <th>Profilo</th>
                  <th>Host</th>
                  <th>Porta</th>
                  <th>Unit ID</th>
                  <th>Polling</th>
                  <th>Reg.</th>
                  <th>Azioni</th>
                </tr>
              </thead>
              <tbody>
                {meters.map((meter) => {
                  const profile = catalog.profiles.find((candidate) => candidate.key === meter.catalog_key);
                  return (
                    <tr key={meter.id} className={meter.id === selectedMeterId ? 'selected-row' : undefined}>
                      <td>{meter.id}</td>
                      <td>{meter.name || 'n/a'}</td>
                      <td>{profile ? `${profile.vendor} ${profile.model}` : (meter.catalog_key ?? 'n/a')}</td>
                      <td>{meter.host ?? 'n/a'}</td>
                      <td>{meter.port ?? 'n/a'}</td>
                      <td>{meter.unit_id ?? 'n/a'}</td>
                      <td>{meter.poll_interval_ms ?? 'n/a'} ms</td>
                      <td>{profile?.registers.length ?? 0}</td>
                      <td>
                        <div className="row-actions">
                          <button
                            className="ghost-button small-button"
                            type="button"
                            onClick={() => openEditMeterModal(meter)}
                          >
                            Modifica
                          </button>
                          <button
                            className="ghost-button small-button button-danger"
                            type="button"
                            onClick={() => void handleDeleteMeter(meter.id)}
                            disabled={saving}
                          >
                            Rimuovi
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </article>

      {meterModalMode ? (
        <ModalShell
          eyebrow={meterModalMode === 'create' ? 'Nuovo' : 'Modifica'}
          title={meterModalMode === 'create' ? 'Nuovo misuratore' : 'Modifica misuratore'}
          onClose={closeMeterModal}
        >
          <div className="stack-form">
            <div className="field-grid">
              <label className="field">
                <span>Codice misuratore</span>
                <input value={meterDraft.id} onChange={(event) => updateMeterDraft({ id: event.target.value })} />
              </label>
              <label className="field">
                <span>Nome</span>
                <input value={meterDraft.name} onChange={(event) => updateMeterDraft({ name: event.target.value })} />
              </label>
            </div>
            <div className="field-grid">
              <label className="field">
                <span>Profilo libreria</span>
                <div className="field-select-shell">
                  <select
                    className="field-select"
                    value={meterDraft.catalog_key ?? ''}
                    onChange={(event) => {
                      const catalogKey = event.target.value || null;
                      const profile = catalog.profiles.find((candidate) => candidate.key === catalogKey);
                      updateMeterDraft({
                        catalog_key: catalogKey,
                        port: profile?.default_port ?? meterDraft.port,
                      });
                    }}
                  >
                    <option value="">Seleziona profilo...</option>
                    {catalog.profiles.map((profile) => (
                      <option key={profile.key} value={profile.key}>
                        {profile.vendor} {profile.model}
                      </option>
                    ))}
                  </select>
                </div>
              </label>
              <label className="field">
                <span>Meter label</span>
                <input
                  value={meterDraft.meter_label ?? ''}
                  onChange={(event) => updateMeterDraft({ meter_label: event.target.value })}
                  placeholder="meter_qg_nord"
                />
              </label>
            </div>
            <div className="field-grid">
              <label className="field">
                <span>Host Modbus</span>
                <input
                  value={meterDraft.host ?? ''}
                  onChange={(event) => updateMeterDraft({ host: event.target.value })}
                  placeholder="192.168.1.50"
                />
              </label>
              <label className="field">
                <span>Porta Modbus</span>
                <input
                  value={meterDraft.port ?? ''}
                  onChange={(event) => updateMeterDraft({ port: parseIntegerOrNull(event.target.value) })}
                  placeholder="502"
                />
              </label>
            </div>
            <div className="field-grid">
              <label className="field">
                <span>Unit ID</span>
                <input
                  value={meterDraft.unit_id ?? ''}
                  onChange={(event) => updateMeterDraft({ unit_id: parseIntegerOrNull(event.target.value) })}
                  placeholder="1"
                />
              </label>
              <label className="field">
                <span>Polling ms</span>
                <input
                  value={meterDraft.poll_interval_ms ?? ''}
                  onChange={(event) => updateMeterDraft({ poll_interval_ms: parseIntegerOrNull(event.target.value) })}
                  placeholder="1000"
                />
              </label>
            </div>
            <div className="field-grid">
              <label className="field">
                <span>Corrente max A</span>
                <input
                  value={meterDraft.max_current_a ?? ''}
                  onChange={(event) => updateMeterDraft({ max_current_a: parseNumberOrNull(event.target.value) })}
                  placeholder="250"
                />
              </label>
              <div className="field">
                <span>Registri profilo</span>
                <input
                  readOnly
                  value={catalog.profiles.find((profile) => profile.key === meterDraft.catalog_key)?.registers.length ?? 0}
                />
              </div>
            </div>
            <label className="field">
              <span>Note</span>
              <textarea
                rows={3}
                value={meterDraft.notes ?? ''}
                onChange={(event) => updateMeterDraft({ notes: event.target.value })}
              />
            </label>

            <div className="modal-actions">
              <button className="ghost-button" type="button" onClick={closeMeterModal} disabled={saving}>
                Annulla
              </button>
              <button className="primary-button" type="button" onClick={handleSubmitMeterModal} disabled={saving}>
                {meterModalMode === 'create' ? 'Aggiungi misuratore' : 'Aggiorna misuratore'}
              </button>
            </div>
          </div>
        </ModalShell>
      ) : null}
    </>
  );
}
