import { FormEvent, useEffect, useMemo, useState } from 'react';
import {
  createBadge,
  createUser,
  fetchBadges,
  fetchStationConnectors,
  fetchStations,
  fetchStationStatus,
  fetchUsers,
  openRealtimeStateSocket,
  setStationBlocked,
  setStationConnectorActive,
  unlockStationConnector,
  setBadgeActive,
  setUserActive,
  updateBadge,
  updateStationLocation,
  updateUser,
  type Badge,
  type ConnectorSummary,
  type RealtimeStateSnapshot,
  type StationSummary,
  type User,
} from '../api';
import type { AppData, BackendStatus, ModalKind, StationStatus } from '../appTypes';

const HEARTBEAT_OFFLINE_AFTER_MS = 2 * 60 * 1000;

function sortStations(stations: StationSummary[]): StationSummary[] {
  return [...stations].sort((left, right) => {
    const leftLabel = left.station_name?.trim() || left.station_id;
    const rightLabel = right.station_name?.trim() || right.station_id;

    const byName = leftLabel.localeCompare(rightLabel, 'it', { sensitivity: 'base' });
    if (byName !== 0) return byName;

    return left.station_id.localeCompare(right.station_id, 'it', { sensitivity: 'base' });
  });
}

function getStationStatus(station: StationSummary): StationStatus {
  const lastSeen = new Date(station.last_seen_at).getTime();
  if (Number.isNaN(lastSeen) || Date.now() - lastSeen > HEARTBEAT_OFFLINE_AFTER_MS) {
    return 'offline';
  }

  if (station.current_error_code && station.current_error_code !== 'NoError') {
    return 'error';
  }

  return 'online';
}

function parseNumberOrNull(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : null;
}

type Options = {
  onCreatedUser?: () => void;
  onCreatedBadge?: () => void;
};

export function useAppState(options: Options = {}) {
  const [modalKind, setModalKind] = useState<ModalKind>(null);

  const [stations, setStations] = useState<StationSummary[]>([]);
  const [stationConnectors, setStationConnectors] = useState<ConnectorSummary[]>([]);
  const [users, setUsers] = useState<User[]>([]);
  const [badges, setBadges] = useState<Badge[]>([]);

  const [selectedStationId, setSelectedStationId] = useState<string | null>(null);
  const [selectedUserId, setSelectedUserId] = useState<number | null>(null);
  const [selectedBadgeId, setSelectedBadgeId] = useState<number | null>(null);

  const [loadingStations, setLoadingStations] = useState(true);
  const [loadingStationConnectors, setLoadingStationConnectors] = useState(false);
  const [loadingUsers, setLoadingUsers] = useState(true);
  const [loadingBadges, setLoadingBadges] = useState(true);

  const [stationError, setStationError] = useState<string | null>(null);
  const [stationConnectorsError, setStationConnectorsError] = useState<string | null>(null);
  const [userError, setUserError] = useState<string | null>(null);
  const [badgeError, setBadgeError] = useState<string | null>(null);
  const [backendStatus, setBackendStatus] = useState<BackendStatus>('connecting');
  const [backendStatusDetail, setBackendStatusDetail] = useState('Connessione iniziale');
  const [lastSyncAt, setLastSyncAt] = useState<string | null>(null);

  const [saving, setSaving] = useState(false);
  const [stationCommandBusy, setStationCommandBusy] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);

  const [userDraft, setUserDraft] = useState({ display_name: '', email: '' });
  const [badgeDraft, setBadgeDraft] = useState({
    badge_code: '',
    label: '',
    user_id: '',
  });
  const [userEditId, setUserEditId] = useState<number | null>(null);
  const [badgeEditId, setBadgeEditId] = useState<number | null>(null);
  const [stationDraft, setStationDraft] = useState({
    station_name: '',
    latitude: '',
    longitude: '',
    location_label: '',
    address: '',
    notes: '',
  });

  function markSyncSuccess(detail?: string) {
    if (detail) {
      setBackendStatus('connected');
      setBackendStatusDetail(detail);
    }
    setLastSyncAt(new Date().toISOString());
  }

  async function loadStations(keepVisible = true) {
    if (!keepVisible) {
      setLoadingStations(true);
    }
    setStationError(null);
    try {
      const data = sortStations(await fetchStations());
      setStations(data);
      setSelectedStationId((current) => current ?? data[0]?.station_id ?? null);
      markSyncSuccess();
    } catch (err) {
      setBackendStatus((current) => (current === 'connected' ? 'degraded' : current));
      setBackendStatusDetail('Errore lettura colonnine');
      setStationError(err instanceof Error ? err.message : 'Errore caricando le colonnine');
    } finally {
      setLoadingStations(false);
    }
  }

  async function loadUsers(keepVisible = true) {
    if (!keepVisible) {
      setLoadingUsers(true);
    }
    setUserError(null);
    try {
      const data = await fetchUsers();
      setUsers(data);
      setSelectedUserId((current) => current ?? data[0]?.id ?? null);
      markSyncSuccess();
    } catch (err) {
      setBackendStatus((current) => (current === 'connected' ? 'degraded' : current));
      setBackendStatusDetail('Errore lettura utenti');
      setUserError(err instanceof Error ? err.message : 'Errore caricando gli utenti');
    } finally {
      setLoadingUsers(false);
    }
  }

  async function loadBadges(keepVisible = true) {
    if (!keepVisible) {
      setLoadingBadges(true);
    }
    setBadgeError(null);
    try {
      const data = await fetchBadges();
      setBadges(data);
      setSelectedBadgeId((current) => current ?? data[0]?.id ?? null);
      markSyncSuccess();
    } catch (err) {
      setBackendStatus((current) => (current === 'connected' ? 'degraded' : current));
      setBackendStatusDetail('Errore lettura badge');
      setBadgeError(err instanceof Error ? err.message : 'Errore caricando i badge');
    } finally {
      setLoadingBadges(false);
    }
  }

  useEffect(() => {
    void loadStations(false);
    void loadUsers(false);
    void loadBadges(false);
  }, []);

  useEffect(() => {
    let reconnectTimer: number | null = null;
    let closed = false;
    let socket: WebSocket | null = null;
    let seenMessage = false;

    function connect() {
      setBackendStatus((current) => (current === 'connected' ? 'reconnecting' : 'connecting'));
      setBackendStatusDetail(seenMessage ? 'Riconnessione realtime' : 'Connessione realtime');
      socket = openRealtimeStateSocket();

      socket.onmessage = (event) => {
        try {
          const snapshot = JSON.parse(event.data) as RealtimeStateSnapshot;
          seenMessage = true;
          const stations = sortStations(snapshot.stations);
          setStations(stations);
          setStationConnectors(snapshot.connectors);
          setSelectedStationId((current) => current ?? stations[0]?.station_id ?? null);
          setStationError(null);
          setStationConnectorsError(null);
          setLoadingStations(false);
          setLoadingStationConnectors(false);
          markSyncSuccess('Realtime attivo');
        } catch (err) {
          setBackendStatus('degraded');
          setBackendStatusDetail('Snapshot realtime non valido');
          setStationError(err instanceof Error ? err.message : 'Realtime state non valido');
        }
      };

      socket.onerror = () => {
        setBackendStatus('degraded');
        setBackendStatusDetail('Realtime non disponibile');
        setStationError('Connessione realtime stato non disponibile');
        void loadStations();
      };

      socket.onclose = () => {
        if (closed) return;
        setBackendStatus(seenMessage ? 'reconnecting' : 'degraded');
        setBackendStatusDetail(seenMessage ? 'Realtime interrotto' : 'Realtime non disponibile');
        reconnectTimer = window.setTimeout(connect, 2000);
      };
    }

    connect();

    return () => {
      closed = true;
      if (reconnectTimer !== null) {
        window.clearTimeout(reconnectTimer);
      }
      socket?.close();
    };
  }, []);

  const selectedStation = useMemo(
    () => stations.find((station) => station.station_id === selectedStationId) ?? stations[0] ?? null,
    [selectedStationId, stations],
  );

  const selectedStationConnectors = useMemo(
    () => stationConnectors.filter((connector) => connector.station_id === selectedStation?.station_id),
    [selectedStation?.station_id, stationConnectors],
  );

  const selectedUser = useMemo(
    () => users.find((user) => user.id === selectedUserId) ?? users[0] ?? null,
    [selectedUserId, users],
  );

  const selectedBadge = useMemo(
    () => badges.find((badge) => badge.id === selectedBadgeId) ?? badges[0] ?? null,
    [selectedBadgeId, badges],
  );

  const selectedUserBadges = useMemo(
    () => badges.filter((badge) => badge.user_id === selectedUser?.id),
    [badges, selectedUser?.id],
  );

  useEffect(() => {
    if (modalKind !== 'station-location' || !selectedStation) return;
    setStationDraft({
      station_name: selectedStation.station_name ?? '',
      latitude: selectedStation.latitude?.toString() ?? '',
      longitude: selectedStation.longitude?.toString() ?? '',
      location_label: selectedStation.location_label ?? '',
      address: selectedStation.address ?? '',
      notes: selectedStation.notes ?? '',
    });
  }, [modalKind, selectedStation]);

  useEffect(() => {
    if (!selectedStation?.station_id) {
      setStationConnectors([]);
      setStationConnectorsError(null);
      return;
    }

    setStationConnectorsError(null);
    void fetchStationConnectors(selectedStation.station_id)
      .then((data) => {
        setStationConnectors((current) => [
          ...current.filter((connector) => connector.station_id !== selectedStation.station_id),
          ...data,
        ]);
      })
      .catch((err) => {
        setStationConnectorsError(err instanceof Error ? err.message : 'Errore caricando i connettori');
      })
      .finally(() => {
        setLoadingStationConnectors(false);
      });
  }, [selectedStation?.station_id]);

  useEffect(() => {
    if (modalKind === 'create-badge') {
      setBadgeDraft((current) => ({
        ...current,
        user_id: current.user_id || (selectedUser ? String(selectedUser.id) : ''),
      }));
    }
  }, [modalKind, selectedUser]);

  useEffect(() => {
    if (modalKind === 'edit-user' && selectedUser) {
      setUserDraft({
        display_name: selectedUser.display_name,
        email: selectedUser.email ?? '',
      });
      setUserEditId(selectedUser.id);
    }
  }, [modalKind, selectedUser]);

  useEffect(() => {
    if (modalKind === 'edit-badge' && selectedBadge) {
      setBadgeDraft({
        badge_code: selectedBadge.badge_code,
        label: selectedBadge.label ?? '',
        user_id: String(selectedBadge.user_id),
      });
      setBadgeEditId(selectedBadge.id);
    }
  }, [modalKind, selectedBadge]);

  const stationStatuses = stations.map((station) => getStationStatus(station));
  const onlineCount = stationStatuses.filter((status) => status === 'online').length;
  const offlineCount = stationStatuses.filter((status) => status === 'offline').length;
  const errorCount = stationStatuses.filter((status) => status === 'error').length;
  const locationCount = stations.filter((station) => station.location_label).length;
  const lastBoot = stations
    .map((station) => station.last_boot_at)
    .filter((value): value is string => Boolean(value))
    .sort()
    .at(-1);

  const activeUsers = users.filter((user) => user.active).length;
  const activeBadges = badges.filter((badge) => badge.active).length;
  const assignedBadges = badges.filter((badge) => badge.user_id != null).length;

  function openModal(kind: ModalKind) {
    setFormError(null);
    setModalKind(kind);

    if (kind === 'create-user') {
      setUserDraft({ display_name: '', email: '' });
      setUserEditId(null);
    }

    if (kind === 'edit-user' && selectedUser) {
      setUserDraft({
        display_name: selectedUser.display_name,
        email: selectedUser.email ?? '',
      });
      setUserEditId(selectedUser.id);
    }

    if (kind === 'create-badge') {
      setBadgeDraft({
        badge_code: '',
        label: '',
        user_id: selectedUser?.id ? String(selectedUser.id) : '',
      });
      setBadgeEditId(null);
    }

    if (kind === 'edit-badge' && selectedBadge) {
      setBadgeDraft({
        badge_code: selectedBadge.badge_code,
        label: selectedBadge.label ?? '',
        user_id: String(selectedBadge.user_id),
      });
      setBadgeEditId(selectedBadge.id);
    }
  }

  function closeModal() {
    if (saving) return;
    setModalKind(null);
    setFormError(null);
  }

  async function handleSave(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSaving(true);
    setFormError(null);

    try {
      if (modalKind === 'create-user') {
        const created = await createUser({
          display_name: userDraft.display_name.trim(),
          email: userDraft.email.trim() ? userDraft.email.trim() : null,
        });
        setUsers((current) => [created, ...current]);
        setSelectedUserId(created.id);
        options.onCreatedUser?.();
      }

      if (modalKind === 'edit-user' && userEditId != null) {
        const updated = await updateUser(userEditId, {
          display_name: userDraft.display_name.trim(),
          email: userDraft.email.trim() ? userDraft.email.trim() : null,
        });
        setUsers((current) => current.map((user) => (user.id === updated.id ? updated : user)));
        setSelectedUserId(updated.id);
      }

      if (modalKind === 'create-badge') {
        const userId = badgeDraft.user_id.trim() ? Number(badgeDraft.user_id) : null;
        if (userId !== null && !Number.isFinite(userId)) {
          throw new Error('Seleziona un utente valido');
        }

        const created = await createBadge({
          user_id: userId,
          badge_code: badgeDraft.badge_code.trim(),
          label: badgeDraft.label.trim() ? badgeDraft.label.trim() : null,
        });
        setBadges((current) => [created, ...current]);
        setSelectedBadgeId(created.id);
        options.onCreatedBadge?.();
      }

      if (modalKind === 'edit-badge' && badgeEditId != null) {
        const userId = badgeDraft.user_id.trim() ? Number(badgeDraft.user_id) : null;
        if (userId !== null && !Number.isFinite(userId)) {
          throw new Error('Seleziona un utente valido');
        }

        const updated = await updateBadge(badgeEditId, {
          user_id: userId,
          badge_code: badgeDraft.badge_code.trim(),
          label: badgeDraft.label.trim() ? badgeDraft.label.trim() : null,
        });
        setBadges((current) => current.map((badge) => (badge.id === updated.id ? updated : badge)));
        setSelectedBadgeId(updated.id);
      }

      if (modalKind === 'station-location' && selectedStation) {
        const updated = await updateStationLocation(selectedStation.station_id, {
          station_name: stationDraft.station_name.trim() ? stationDraft.station_name.trim() : null,
          latitude: parseNumberOrNull(stationDraft.latitude),
          longitude: parseNumberOrNull(stationDraft.longitude),
          location_label: stationDraft.location_label.trim() ? stationDraft.location_label.trim() : null,
          address: stationDraft.address.trim() ? stationDraft.address.trim() : null,
          notes: stationDraft.notes.trim() ? stationDraft.notes.trim() : null,
        });
        setStations((current) => sortStations(
          current.map((station) => (station.station_id === updated.station_id ? updated : station)),
        ));
        setSelectedStationId(updated.station_id);
      }

      setModalKind(null);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : 'Errore salvando il dato');
    } finally {
      setSaving(false);
    }
  }

  async function toggleUserActive(user: User) {
    await setUserActive(user.id, !user.active);
    void loadUsers();
  }

  async function toggleBadgeActive(badge: Badge) {
    await setBadgeActive(badge.id, !badge.active);
    void loadBadges();
  }

  async function syncStationStatus(stationId: string) {
    const updated = await fetchStationStatus(stationId);
    setStations((current) => sortStations(
      current.map((station) => (station.station_id === updated.station_id ? updated : station)),
    ));
    markSyncSuccess();
    if (selectedStationId === updated.station_id) {
      setSelectedStationId(updated.station_id);
    }
  }

  async function syncStationConnectors(stationId: string) {
    const connectors = await fetchStationConnectors(stationId);
    setStationConnectors(connectors);
    markSyncSuccess();
  }

  async function syncStationSnapshot(stationId: string) {
    await syncStationStatus(stationId);
    await syncStationConnectors(stationId);
  }

  function scheduleStationSnapshotRefresh(stationId: string, delayMs: number) {
    window.setTimeout(() => {
      void syncStationSnapshot(stationId).catch(() => {});
    }, delayMs);
  }

  async function refreshStationStatus(stationId: string) {
    setStationCommandBusy(true);
    setFormError(null);
    try {
      await syncStationStatus(stationId);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : 'Errore aggiornando lo stato della colonnina');
    } finally {
      setStationCommandBusy(false);
    }
  }

  async function toggleStationBlocked(stationId: string, blocked: boolean) {
    setStationCommandBusy(true);
    setFormError(null);
    try {
      await setStationBlocked(stationId, blocked);
      await syncStationStatus(stationId);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : 'Errore aggiornando il blocco della colonnina');
    } finally {
      setStationCommandBusy(false);
    }
  }

  async function toggleStationConnectorActive(stationId: string, connectorId: number, active: boolean) {
    setStationCommandBusy(true);
    setFormError(null);
    try {
      await setStationConnectorActive(stationId, connectorId, active);
      await syncStationSnapshot(stationId);
      scheduleStationSnapshotRefresh(stationId, 1200);
      scheduleStationSnapshotRefresh(stationId, 3000);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : 'Errore aggiornando il connettore');
    } finally {
      setStationCommandBusy(false);
    }
  }

  async function unlockStationConnectorCommand(stationId: string, connectorId: number) {
    setStationCommandBusy(true);
    setFormError(null);
    try {
      await unlockStationConnector(stationId, connectorId);
      await syncStationSnapshot(stationId);
      scheduleStationSnapshotRefresh(stationId, 1200);
      scheduleStationSnapshotRefresh(stationId, 3000);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : 'Errore sbloccando il connettore della colonnina');
    } finally {
      setStationCommandBusy(false);
    }
  }

  const data: AppData = {
    stations,
    stationConnectors,
    users,
    badges,
    selectedStation,
    selectedUser,
    selectedBadge,
    selectedUserBadges,
    loadingStations,
    loadingStationConnectors,
    loadingUsers,
    loadingBadges,
    stationError,
    stationConnectorsError,
    userError,
    badgeError,
    onlineCount,
    offlineCount,
    errorCount,
    locationCount,
    lastBoot,
    activeUsers,
    activeBadges,
    assignedBadges,
    backendStatus,
    backendStatusDetail,
    lastSyncAt,
  };

  return {
    modalKind,
    openModal,
    closeModal,
    handleSave,
    saving,
    formError,
    data,
    selectedStation,
    selectedStationConnectors,
    users,
    userDraft,
    badgeDraft,
    stationDraft,
    setUserDraft,
    setBadgeDraft,
    setStationDraft,
    stationCommandBusy,
    selectStation: setSelectedStationId,
    refreshStationStatus,
    toggleStationBlocked,
    toggleStationConnectorActive,
    unlockStationConnector: unlockStationConnectorCommand,
    selectUser: setSelectedUserId,
    selectBadge: setSelectedBadgeId,
    toggleUserActive,
    toggleBadgeActive,
  };
}
