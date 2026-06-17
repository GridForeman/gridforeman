import type { StationSummary } from './api';

export function isStationBlocked(station: Pick<StationSummary, 'blocked' | 'current_status'>): boolean {
  return station.blocked || station.current_status === 'Unavailable';
}

export function stationAccessLabel(station: Pick<StationSummary, 'blocked' | 'current_status'>): string {
  return isStationBlocked(station) ? 'Ricarica bloccata' : 'Ricarica consentita';
}
