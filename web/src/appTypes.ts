import type { Badge, ConnectorSummary, SiteConfigSnapshot, StationConfigurationSnapshot, StationSummary, User } from './api';

export type AppRoute = '/site' | '/energy-meters' | '/stations' | '/users' | '/badges' | '/events' | '/transactions';
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
  stationConfiguration: StationConfigurationSnapshot | null;
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
  fetchStationConfiguration: (stationId: string) => Promise<void>;
  remoteStartStationConnector: (stationId: string, connectorId: number, badgeCode: string) => Promise<void>;
  remoteStopStationConnector: (stationId: string, connectorId: number) => Promise<void>;
  setConnectorAutoRemoteStartBadge: (stationId: string, connectorId: number, badgeCode: string | null) => Promise<void>;
  toggleStationConnectorActive: (stationId: string, connectorId: number, active: boolean) => Promise<void>;
  unlockStationConnector: (stationId: string, connectorId: number) => Promise<void>;
  selectUser: (userId: number) => void;
  selectBadge: (badgeId: number) => void;
  toggleUserActive: (user: User) => Promise<void>;
  toggleBadgeActive: (badge: Badge) => Promise<void>;
};
