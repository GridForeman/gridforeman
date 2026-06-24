import type { Dispatch, FormEvent, SetStateAction } from 'react';
import type { Badge, ConnectorSummary, StationConfigurationSnapshot, StationSummary, User } from '../../api';

export type UserDraft = {
  display_name: string;
  email: string;
};

export type BadgeDraft = {
  badge_code: string;
  label: string;
  user_id: string;
};

export type StationDraft = {
  station_name: string;
  latitude: string;
  longitude: string;
  location_label: string;
  address: string;
  notes: string;
};

export type ModalFormSharedProps = {
  saving: boolean;
  formError: string | null;
  closeModal: () => void;
  handleSave: (event: FormEvent<HTMLFormElement>) => void;
};

export type UserFormModalBodyProps = ModalFormSharedProps & {
  userDraft: UserDraft;
  setUserDraft: Dispatch<SetStateAction<UserDraft>>;
};

export type BadgeFormModalBodyProps = ModalFormSharedProps & {
  users: User[];
  badgeDraft: BadgeDraft;
  setBadgeDraft: Dispatch<SetStateAction<BadgeDraft>>;
};

export type StationLocationModalBodyProps = ModalFormSharedProps & {
  selectedStation: StationSummary | null;
  stationDraft: StationDraft;
  setStationDraft: Dispatch<SetStateAction<StationDraft>>;
};

export type StationControlsModalBodyProps = {
  selectedStation: StationSummary | null;
  selectedStationConnectors: ConnectorSummary[];
  badges: Badge[];
  stationConfiguration: StationConfigurationSnapshot | null;
  stationCommandBusy: boolean;
  formError: string | null;
  loadingStationConnectors: boolean;
  stationConnectorsError: string | null;
  closeModal: () => void;
  refreshStationStatus: (stationId: string) => Promise<void>;
  toggleStationBlocked: (stationId: string, blocked: boolean) => Promise<void>;
  fetchStationConfiguration: (stationId: string) => Promise<void>;
  remoteStartStationConnector: (stationId: string, connectorId: number, badgeCode: string) => Promise<void>;
  remoteStopStationConnector: (stationId: string, connectorId: number) => Promise<void>;
  setConnectorAutoRemoteStartBadge: (stationId: string, connectorId: number, badgeCode: string | null) => Promise<void>;
  toggleStationConnectorActive: (stationId: string, connectorId: number, active: boolean) => Promise<void>;
  unlockStationConnector: (stationId: string, connectorId: number) => Promise<void>;
};
