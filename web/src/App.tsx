import { Navigate, Route, Routes, useLocation, useNavigate } from 'react-router-dom';
import { AppFrame } from './components/AppFrame';
import { AppModal } from './components/AppModal';
import { BadgesPage } from './pages/BadgesPage';
import { EnergyMetersPage } from './pages/EnergyMetersPage';
import { EventsPage } from './pages/EventsPage';
import { SitePage } from './pages/SitePage';
import { StationsPage } from './pages/StationsPage';
import { TransactionsPage } from './pages/TransactionsPage';
import { UsersPage } from './pages/UsersPage';
import { useAppState } from './hooks/useAppState';
import type { AppRoute } from './appTypes';

const routeTitles: Record<AppRoute, string> = {
  '/site': 'Impianto',
  '/energy-meters': 'Misuratori',
  '/stations': 'Colonnine',
  '/users': 'Utenti',
  '/badges': 'Badge',
  '/events': 'Eventi',
  '/transactions': 'Transazioni',
};

function timeAgo(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  const minutes = Math.max(0, Math.round((Date.now() - date.getTime()) / 60000));
  if (minutes < 1) return 'adesso';
  if (minutes < 60) return `${minutes} min fa`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} h fa`;
  return `${Math.round(hours / 24)} g fa`;
}

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat('it-CH', {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(date);
}

function formatStationName(station: { station_name: string | null; station_id: string }): string {
  return station.station_name?.trim() || station.station_id;
}

function formatLocation(station: { location_label: string | null }): string {
  return station.location_label ?? 'Non assegnata';
}

const HEARTBEAT_OFFLINE_AFTER_MS = 2 * 60 * 1000;

function getStationStatus(station: { last_seen_at: string; current_error_code: string | null }) {
  const lastSeen = new Date(station.last_seen_at).getTime();
  if (Number.isNaN(lastSeen) || Date.now() - lastSeen > HEARTBEAT_OFFLINE_AFTER_MS) {
    return 'offline';
  }

  if (station.current_error_code && station.current_error_code !== 'NoError') {
    return 'error';
  }

  return 'online';
}

export default function App() {
  const navigate = useNavigate();
  const location = useLocation();
  const app = useAppState({
    onCreatedUser: () => navigate('/users'),
    onCreatedBadge: () => navigate('/badges'),
  });

  const title = routeTitles[location.pathname as AppRoute] ?? 'Colonnine';

  return (
    <>
      <AppFrame
        title={title}
        onCreateUser={() => app.openModal('create-user')}
        onCreateBadge={() => app.openModal('create-badge')}
        backendStatus={app.data.backendStatus}
        backendStatusDetail={app.data.backendStatusDetail}
        lastSyncLabel={app.data.lastSyncAt ? timeAgo(app.data.lastSyncAt) : 'mai'}
      >
        <Routes>
          <Route
            path="/"
            element={<Navigate to="/site" replace />}
          />
          <Route
            path="/site"
            element={<SitePage stations={app.data.stations} />}
          />
          <Route
            path="/energy-meters"
            element={<EnergyMetersPage />}
          />
          <Route
            path="/stations"
            element={(
              <StationsPage
                stations={app.data.stations}
                selectedStation={app.data.selectedStation}
                stationConnectors={app.data.stationConnectors}
                loadingStations={app.data.loadingStations}
                loadingStationConnectors={app.data.loadingStationConnectors}
                stationError={app.data.stationError}
                stationConnectorsError={app.data.stationConnectorsError}
                onlineCount={app.data.onlineCount}
                offlineCount={app.data.offlineCount}
                errorCount={app.data.errorCount}
                locationCount={app.data.locationCount}
                lastBoot={app.data.lastBoot}
                actions={{
                  openModal: app.openModal,
                  selectStation: app.selectStation,
                }}
                timeAgo={timeAgo}
                formatStationName={formatStationName}
                formatLocation={formatLocation}
                getStationStatus={getStationStatus}
              />
            )}
          />
          <Route
            path="/users"
            element={(
              <UsersPage
                users={app.data.users}
                loadingUsers={app.data.loadingUsers}
                userError={app.data.userError}
                actions={{
                  openModal: app.openModal,
                  selectUser: app.selectUser,
                  toggleUserActive: app.toggleUserActive,
                }}
                formatDate={formatDate}
              />
            )}
          />
          <Route
            path="/badges"
            element={(
              <BadgesPage
                badges={app.data.badges}
                users={app.data.users}
                selectedBadge={app.data.selectedBadge}
                loadingBadges={app.data.loadingBadges}
                badgeError={app.data.badgeError}
                actions={{
                  openModal: app.openModal,
                  selectBadge: app.selectBadge,
                  toggleBadgeActive: app.toggleBadgeActive,
                }}
              />
            )}
          />
          <Route
            path="/events"
            element={(
              <EventsPage stations={app.data.stations} />
            )}
          />
          <Route
            path="/transactions"
            element={(
              <TransactionsPage
                stations={app.data.stations}
                users={app.data.users}
                badges={app.data.badges}
              />
            )}
          />
          <Route path="*" element={<Navigate to="/site" replace />} />
        </Routes>
      </AppFrame>

      <AppModal
        modalKind={app.modalKind}
        selectedStation={app.selectedStation}
        selectedStationConnectors={app.selectedStationConnectors}
        stationConfiguration={app.data.stationConfiguration}
        saving={app.saving}
        stationCommandBusy={app.stationCommandBusy}
        formError={app.formError}
        loadingStationConnectors={app.data.loadingStationConnectors}
        stationConnectorsError={app.data.stationConnectorsError}
        users={app.users}
        badges={app.data.badges}
        userDraft={app.userDraft}
        badgeDraft={app.badgeDraft}
        stationDraft={app.stationDraft}
        closeModal={app.closeModal}
        handleSave={app.handleSave}
        refreshStationStatus={app.refreshStationStatus}
        toggleStationBlocked={app.toggleStationBlocked}
        fetchStationConfiguration={app.fetchStationConfiguration}
        remoteStartStationConnector={app.remoteStartStationConnector}
        remoteStopStationConnector={app.remoteStopStationConnector}
        setConnectorAutoRemoteStartBadge={app.setConnectorAutoRemoteStartBadge}
        toggleStationConnectorActive={app.toggleStationConnectorActive}
        unlockStationConnector={app.unlockStationConnector}
        setUserDraft={app.setUserDraft}
        setBadgeDraft={app.setBadgeDraft}
        setStationDraft={app.setStationDraft}
      />
    </>
  );
}
