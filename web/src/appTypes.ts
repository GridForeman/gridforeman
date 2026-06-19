import type { Badge, ConnectorSummary, SiteConfigSnapshot, StationSummary, User } from './api';

export type AppRoute = '/site' | '/stations' | '/users' | '/badges' | '/events' | '/transactions';
export type StationStatus = 'online' | 'offline' | 'error';
export type BackendStatus = 'connecting' | 'connected' | 'reconnecting' | 'degraded';
export type ModalKind =
  | 'station-location'
  | 'station-controls'
  | 'create-user'
  | 'edit-user'
  | 'create-badge'
  | 'edit-badge'
  | null;

export type AppData = {
  stations: StationSummary[];
  siteConfig?: SiteConfigSnapshot;
  users: User[];
  badges: Badge[];
  stationConnectors: ConnectorSummary[];
  selectedStation: StationSummary | null;
  selectedUser: User | null;
  selectedBadge: Badge | null;
  selectedUserBadges: Badge[];
  loadingStations: boolean;
  loadingUsers: boolean;
  loadingBadges: boolean;
  loadingStationConnectors: boolean;
  stationError: string | null;
  stationConnectorsError: string | null;
  userError: string | null;
  badgeError: string | null;
  onlineCount: number;
  offlineCount: number;
  errorCount: number;
  locationCount: number;
  lastBoot: string | null;
  activeUsers: number;
  activeBadges: number;
  assignedBadges: number;
  backendStatus: BackendStatus;
  backendStatusDetail: string;
  lastSyncAt: string | null;
};

export type AppActions = {
  openModal: (kind: ModalKind) => void;
  selectStation: (stationId: string) => void;
  refreshStationStatus: (stationId: string) => Promise<void>;
  toggleStationBlocked: (stationId: string, blocked: boolean) => Promise<void>;
  toggleStationConnectorActive: (stationId: string, connectorId: number, active: boolean) => Promise<void>;
  unlockStationConnector: (stationId: string, connectorId: number) => Promise<void>;
  selectUser: (userId: number) => void;
  selectBadge: (badgeId: number) => void;
  toggleUserActive: (user: User) => Promise<void>;
  toggleBadgeActive: (badge: Badge) => Promise<void>;
};
