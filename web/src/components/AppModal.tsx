import type { Dispatch, FormEvent, SetStateAction } from 'react';
import type { ConnectorSummary, StationSummary, User } from '../api';
import type { ModalKind } from '../appTypes';
import { BadgeFormModalBody } from './modals/BadgeFormModalBody';
import { ModalShell } from './modals/ModalShell';
import { StationControlsModalBody } from './modals/StationControlsModalBody';
import { StationLocationModalBody } from './modals/StationLocationModalBody';
import { UserFormModalBody } from './modals/UserFormModalBody';
import type { BadgeDraft, StationDraft, UserDraft } from './modals/modalTypes';

type Props = {
  modalKind: ModalKind;
  selectedStation: StationSummary | null;
  selectedStationConnectors: ConnectorSummary[];
  saving: boolean;
  stationCommandBusy: boolean;
  formError: string | null;
  loadingStationConnectors: boolean;
  stationConnectorsError: string | null;
  users: User[];
  userDraft: UserDraft;
  badgeDraft: BadgeDraft;
  stationDraft: StationDraft;
  closeModal: () => void;
  handleSave: (event: FormEvent<HTMLFormElement>) => void;
  refreshStationStatus: (stationId: string) => Promise<void>;
  toggleStationBlocked: (stationId: string, blocked: boolean) => Promise<void>;
  toggleStationConnectorActive: (stationId: string, connectorId: number, active: boolean) => Promise<void>;
  unlockStationConnector: (stationId: string, connectorId: number) => Promise<void>;
  setUserDraft: Dispatch<SetStateAction<UserDraft>>;
  setBadgeDraft: Dispatch<SetStateAction<BadgeDraft>>;
  setStationDraft: Dispatch<SetStateAction<StationDraft>>;
};

const modalMeta: Record<Exclude<ModalKind, null>, { eyebrow: string; title: string }> = {
  'create-user': { eyebrow: 'Nuovo', title: 'Nuovo utente' },
  'edit-user': { eyebrow: 'Modifica', title: 'Modifica utente' },
  'create-badge': { eyebrow: 'Nuovo', title: 'Nuovo badge' },
  'edit-badge': { eyebrow: 'Modifica', title: 'Modifica badge' },
  'station-controls': { eyebrow: 'Comandi', title: 'Gestione colonnina' },
  'station-location': { eyebrow: 'Modifica', title: 'Posizione colonnina' },
};

export function AppModal({
  modalKind,
  selectedStation,
  selectedStationConnectors,
  saving,
  stationCommandBusy,
  formError,
  loadingStationConnectors,
  stationConnectorsError,
  users,
  userDraft,
  badgeDraft,
  stationDraft,
  closeModal,
  handleSave,
  refreshStationStatus,
  toggleStationBlocked,
  toggleStationConnectorActive,
  unlockStationConnector,
  setUserDraft,
  setBadgeDraft,
  setStationDraft,
}: Props) {
  if (!modalKind) return null;

  const { eyebrow, title } = modalMeta[modalKind];

  let body = null;
  if (modalKind === 'create-user' || modalKind === 'edit-user') {
    body = (
      <UserFormModalBody
        saving={saving}
        formError={formError}
        closeModal={closeModal}
        handleSave={handleSave}
        userDraft={userDraft}
        setUserDraft={setUserDraft}
      />
    );
  } else if (modalKind === 'create-badge' || modalKind === 'edit-badge') {
    body = (
      <BadgeFormModalBody
        saving={saving}
        formError={formError}
        closeModal={closeModal}
        handleSave={handleSave}
        users={users}
        badgeDraft={badgeDraft}
        setBadgeDraft={setBadgeDraft}
      />
    );
  } else if (modalKind === 'station-location') {
    body = (
      <StationLocationModalBody
        saving={saving}
        formError={formError}
        selectedStation={selectedStation}
        closeModal={closeModal}
        handleSave={handleSave}
        stationDraft={stationDraft}
        setStationDraft={setStationDraft}
      />
    );
  } else if (modalKind === 'station-controls') {
    body = (
      <StationControlsModalBody
        selectedStation={selectedStation}
        selectedStationConnectors={selectedStationConnectors}
        stationCommandBusy={stationCommandBusy}
        formError={formError}
        loadingStationConnectors={loadingStationConnectors}
        stationConnectorsError={stationConnectorsError}
        closeModal={closeModal}
        refreshStationStatus={refreshStationStatus}
        toggleStationBlocked={toggleStationBlocked}
        toggleStationConnectorActive={toggleStationConnectorActive}
        unlockStationConnector={unlockStationConnector}
      />
    );
  }

  return (
    <ModalShell eyebrow={eyebrow} title={title} onClose={closeModal}>
      {body}
    </ModalShell>
  );
}
