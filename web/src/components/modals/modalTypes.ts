import type { Dispatch, FormEvent, SetStateAction } from 'react';
import type { ConnectorSummary, StationSummary, User } from '../../api';

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
  stationCommandBusy: boolean;
  formError: string | null;
  loadingStationConnectors: boolean;
  stationConnectorsError: string | null;
  closeModal: () => void;
  refreshStationStatus: (stationId: string) => Promise<void>;
  toggleStationBlocked: (stationId: string, blocked: boolean) => Promise<void>;
  toggleStationConnectorActive: (stationId: string, connectorId: number, active: boolean) => Promise<void>;
  unlockStationConnector: (stationId: string, connectorId: number) => Promise<void>;
};
