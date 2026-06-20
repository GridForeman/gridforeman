export type StationSummary = {
  station_id: string;
  station_name: string | null;
  blocked: boolean;
  ocpp_version: string;
  peer_addr: string;
  first_seen_at: string;
  last_seen_at: string;
  last_boot_at: string | null;
  boot_count: number;
  latitude: number | null;
  longitude: number | null;
  location_label: string | null;
  address: string | null;
  notes: string | null;
  location_updated_at: string | null;
  current_status: string | null;
  current_error_code: string | null;
  current_connector_id: number | null;
  current_evse_id: number | null;
  current_status_at: string | null;
  updated_at: string;
};

export type ConnectorSummary = {
  station_id: string;
  connector_id: number;
  evse_id: number | null;
  active: boolean;
  current_status: string | null;
  current_error_code: string | null;
  current_status_at: string | null;
  active_transaction_id: number | null;
  active_transaction_ref: string | null;
  updated_at: string;
};

export type RealtimeStateSnapshot = {
  stations: StationSummary[];
  connectors: ConnectorSummary[];
};

export type SiteEnergyMeter = {
  id: string;
  name: string;
  catalog_key: string | null;
  host: string | null;
  port: number | null;
  unit_id: number | null;
  poll_interval_ms: number | null;
  meter_label: string | null;
  max_current_a: number | null;
  notes: string | null;
};

export type EnergyMeter = SiteEnergyMeter & {
  created_at: string;
  updated_at: string;
};

export type SiteManagementGroup = {
  id: string;
  name: string;
  control_mode: string;
  energy_meter_ids: string[];
  station_ids: string[];
  notes: string | null;
};

export type SiteConfigSnapshot = {
  site_name: string | null;
  timezone: string;
  operator_name: string | null;
  notes: string | null;
  energy_meters: SiteEnergyMeter[];
  management_groups: SiteManagementGroup[];
  updated_at: string | null;
};

export type EnergyMeterCatalogRegister = {
  metric_key: string;
  address: number;
  length: number;
  function: string;
  data_type: string;
  endianness: string;
  scale: number;
  unit: string;
};

export type EnergyMeterCatalogProfile = {
  key: string;
  vendor: string;
  model: string;
  transport: string;
  default_port: number;
  registers: EnergyMeterCatalogRegister[];
  notes: string | null;
};

export type EnergyMeterCatalog = {
  profiles: EnergyMeterCatalogProfile[];
};

export async function fetchStations(): Promise<StationSummary[]> {
  const response = await fetch('/api/stations', { cache: 'no-store' });
  if (!response.ok) {
    throw new Error(`GET /api/stations failed with ${response.status}`);
  }

  return response.json();
}

export async function updateStationLocation(
  stationId: string,
  payload: {
    station_name: string | null;
    latitude: number | null;
    longitude: number | null;
    location_label: string | null;
    address: string | null;
    notes: string | null;
  },
): Promise<StationSummary> {
  const response = await fetch(`/api/stations/${stationId}/location`, {
    method: 'PATCH',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(payload),
  });

  if (!response.ok) {
    throw new Error(`PATCH /api/stations/${stationId}/location failed with ${response.status}`);
  }

  return response.json();
}

export async function fetchSiteConfig(): Promise<SiteConfigSnapshot> {
  const response = await fetch('/api/site-config', { cache: 'no-store' });
  if (!response.ok) {
    throw new Error(`GET /api/site-config failed with ${response.status}`);
  }

  return response.json();
}

export async function fetchEnergyMeterCatalog(): Promise<EnergyMeterCatalog> {
  const response = await fetch('/api/energy-meter-catalog', { cache: 'no-store' });
  if (!response.ok) {
    throw new Error(`GET /api/energy-meter-catalog failed with ${response.status}`);
  }

  return response.json();
}

export async function fetchEnergyMeters(): Promise<EnergyMeter[]> {
  const response = await fetch('/api/energy-meters', { cache: 'no-store' });
  if (!response.ok) {
    throw new Error(`GET /api/energy-meters failed with ${response.status}`);
  }

  return response.json();
}

export async function createEnergyMeter(payload: SiteEnergyMeter): Promise<EnergyMeter> {
  const response = await fetch('/api/energy-meters', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(payload),
  });

  if (!response.ok) {
    const detail = await response.text();
    throw new Error(
      detail.trim()
        ? `POST /api/energy-meters failed with ${response.status}: ${detail}`
        : `POST /api/energy-meters failed with ${response.status}`,
    );
  }

  return response.json();
}

export async function updateEnergyMeterRecord(meterId: string, payload: SiteEnergyMeter): Promise<EnergyMeter> {
  const response = await fetch(`/api/energy-meters/${meterId}`, {
    method: 'PATCH',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(payload),
  });

  if (!response.ok) {
    const detail = await response.text();
    throw new Error(
      detail.trim()
        ? `PATCH /api/energy-meters/${meterId} failed with ${response.status}: ${detail}`
        : `PATCH /api/energy-meters/${meterId} failed with ${response.status}`,
    );
  }

  return response.json();
}

export async function deleteEnergyMeter(meterId: string): Promise<void> {
  const response = await fetch(`/api/energy-meters/${meterId}`, {
    method: 'DELETE',
  });

  if (!response.ok) {
    const detail = await response.text();
    throw new Error(
      detail.trim()
        ? `DELETE /api/energy-meters/${meterId} failed with ${response.status}: ${detail}`
        : `DELETE /api/energy-meters/${meterId} failed with ${response.status}`,
    );
  }
}

export async function updateSiteConfig(payload: SiteConfigSnapshot): Promise<SiteConfigSnapshot> {
  const response = await fetch('/api/site-config', {
    method: 'PUT',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(payload),
  });

  if (!response.ok) {
    const detail = await response.text();
    throw new Error(
      detail.trim()
        ? `PUT /api/site-config failed with ${response.status}: ${detail}`
        : `PUT /api/site-config failed with ${response.status}`,
    );
  }

  return response.json();
}

export async function fetchStationStatus(stationId: string): Promise<StationSummary> {
  const response = await fetch(`/api/stations/${stationId}/status`, { cache: 'no-store' });
  if (!response.ok) {
    throw new Error(`GET /api/stations/${stationId}/status failed with ${response.status}`);
  }

  return response.json();
}

export async function fetchStationConnectors(stationId: string): Promise<ConnectorSummary[]> {
  const response = await fetch(`/api/stations/${stationId}/connectors`, { cache: 'no-store' });
  if (!response.ok) {
    throw new Error(`GET /api/stations/${stationId}/connectors failed with ${response.status}`);
  }

  return response.json();
}

export function openRealtimeStateSocket(): WebSocket {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  return new WebSocket(`${protocol}//${window.location.host}/api/state/ws`);
}

export async function setStationBlocked(stationId: string, blocked: boolean): Promise<void> {
  const response = await fetch(`/api/stations/${stationId}/blocked`, {
    method: 'PATCH',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ blocked }),
  });

  if (!response.ok) {
    throw new Error(`PATCH /api/stations/${stationId}/blocked failed with ${response.status}`);
  }
}

export async function setStationConnectorActive(stationId: string, connectorId: number, active: boolean): Promise<void> {
  const response = await fetch(`/api/stations/${stationId}/connectors/${connectorId}/active`, {
    method: 'PATCH',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ active }),
  });

  if (!response.ok) {
    const detail = await response.text();
    throw new Error(
      detail.trim()
        ? `PATCH /api/stations/${stationId}/connectors/${connectorId}/active failed with ${response.status}: ${detail}`
        : `PATCH /api/stations/${stationId}/connectors/${connectorId}/active failed with ${response.status}`,
    );
  }
}

export async function unlockStationConnector(stationId: string, connectorId: number): Promise<void> {
  const response = await fetch(`/api/stations/${stationId}/connectors/${connectorId}/unlock`, {
    method: 'PATCH',
  });

  if (!response.ok) {
    const detail = await response.text();
    throw new Error(
      detail.trim()
        ? `PATCH /api/stations/${stationId}/connectors/${connectorId}/unlock failed with ${response.status}: ${detail}`
        : `PATCH /api/stations/${stationId}/connectors/${connectorId}/unlock failed with ${response.status}`,
    );
  }
}

export type User = {
  id: number;
  display_name: string;
  email: string | null;
  active: boolean;
  created_at: string;
  updated_at: string;
};

export async function fetchUsers(): Promise<User[]> {
  const response = await fetch('/api/users', { cache: 'no-store' });
  if (!response.ok) {
    throw new Error(`GET /api/users failed with ${response.status}`);
  }

  return response.json();
}

export async function createUser(payload: { display_name: string; email: string | null }): Promise<User> {
  const response = await fetch('/api/users', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(payload),
  });

  if (!response.ok) {
    throw new Error(`POST /api/users failed with ${response.status}`);
  }

  return response.json();
}

export async function updateUser(
  userId: number,
  payload: { display_name: string; email: string | null },
): Promise<User> {
  const response = await fetch(`/api/users/${userId}`, {
    method: 'PATCH',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(payload),
  });

  if (!response.ok) {
    throw new Error(`PATCH /api/users/${userId} failed with ${response.status}`);
  }

  return response.json();
}

export async function setUserActive(userId: number, active: boolean): Promise<void> {
  const response = await fetch(`/api/users/${userId}/active`, {
    method: 'PATCH',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ active }),
  });

  if (!response.ok) {
    throw new Error(`PATCH /api/users/${userId}/active failed with ${response.status}`);
  }
}

export type Badge = {
  id: number;
  user_id: number | null;
  badge_code: string;
  label: string | null;
  active: boolean;
  created_at: string;
  updated_at: string;
};

export type OcppEvent = {
  message_id: string;
  station_id: string;
  ocpp_version: string;
  peer_addr: string;
  direction: string;
  message_type: number | null;
  unique_id: string | null;
  action: string | null;
  raw_text: string;
  payload: string | null;
  parse_status: string;
  error: string | null;
  received_at: string;
};

export type ChargingTransaction = {
  id: number;
  station_id: string;
  ocpp_version: string;
  ocpp_transaction_id: number | null;
  ocpp_transaction_ref: string | null;
  connector_id: number | null;
  evse_id: number | null;
  user_id: number | null;
  badge_id: number | null;
  badge_code: string | null;
  status: string;
  started_at: string;
  ended_at: string | null;
  meter_start_wh: number | null;
  meter_stop_wh: number | null;
  last_meter_wh: number | null;
  energy_wh: number | null;
  stop_reason: string | null;
  created_at: string;
  updated_at: string;
};

export async function fetchBadges(): Promise<Badge[]> {
  const response = await fetch('/api/badges', { cache: 'no-store' });
  if (!response.ok) {
    throw new Error(`GET /api/badges failed with ${response.status}`);
  }

  return response.json();
}

export async function createBadge(payload: {
  user_id: number | null;
  badge_code: string;
  label: string | null;
}): Promise<Badge> {
  const response = await fetch('/api/badges', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(payload),
  });

  if (!response.ok) {
    throw new Error(`POST /api/badges failed with ${response.status}`);
  }

  return response.json();
}

export async function updateBadge(
  badgeId: number,
  payload: {
    user_id: number | null;
    badge_code: string;
    label: string | null;
  },
): Promise<Badge> {
  const response = await fetch(`/api/badges/${badgeId}`, {
    method: 'PATCH',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(payload),
  });

  if (!response.ok) {
    throw new Error(`PATCH /api/badges/${badgeId} failed with ${response.status}`);
  }

  return response.json();
}

export async function setBadgeActive(badgeId: number, active: boolean): Promise<void> {
  const response = await fetch(`/api/badges/${badgeId}/active`, {
    method: 'PATCH',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ active }),
  });

  if (!response.ok) {
    throw new Error(`PATCH /api/badges/${badgeId}/active failed with ${response.status}`);
  }
}

export async function fetchEvents(limit = 200, stationName?: string | null): Promise<OcppEvent[]> {
  const url = new URL('/api/events', window.location.origin);
  url.searchParams.set('limit', String(limit));
  if (stationName) {
    url.searchParams.set('station_name', stationName);
  }

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`GET /api/events failed with ${response.status}`);
  }

  return response.json();
}

export async function fetchTransactions(limit = 200): Promise<ChargingTransaction[]> {
  const url = new URL('/api/transactions', window.location.origin);
  url.searchParams.set('limit', String(limit));

  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`GET /api/transactions failed with ${response.status}`);
  }

  return response.json();
}
