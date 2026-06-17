import type { StationLocationModalBodyProps } from './modalTypes';

export function StationLocationModalBody({
  saving,
  formError,
  selectedStation,
  closeModal,
  handleSave,
  stationDraft,
  setStationDraft,
}: StationLocationModalBodyProps) {
  return (
    <form className="stack-form modal-form" onSubmit={handleSave}>
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
