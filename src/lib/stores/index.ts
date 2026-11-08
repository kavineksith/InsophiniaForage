// Reactive stores backed by the Tauri API. Components subscribe to these
// instead of calling invoke() directly — this gives us one place to
// centralize polling, error handling, and event subscriptions.

import { writable, type Readable } from 'svelte/store';
import * as api from '$lib/api';
import type { BinaryStatus, Project, ServiceStatus, Settings } from '$lib/types';

// ---- Services store: polled every 2s + refreshed on events ---------------

interface ServicesState {
  list: ServiceStatus[];
  loading: boolean;
  error: string | null;
}

function createServicesStore() {
  const { subscribe, set, update } = writable<ServicesState>({
    list: [],
    loading: false,
    error: null
  });

  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let unlistenStatus: (() => void) | null = null;

  async function refresh() {
    update((s) => ({ ...s, loading: true, error: null }));
    try {
      const list = await api.serviceStatus();
      set({ list, loading: false, error: null });
    } catch (e) {
      update((s) => ({ ...s, loading: false, error: String(e) }));
    }
  }

  async function start(name: string) {
    try {
      await api.startService(name);
      await refresh();
    } catch (e) {
      update((s) => ({ ...s, error: String(e) }));
    }
  }

  async function stop(name: string) {
    try {
      await api.stopService(name);
      await refresh();
    } catch (e) {
      update((s) => ({ ...s, error: String(e) }));
    }
  }

  async function startAll() {
    try {
      await api.startService('all');
      await refresh();
    } catch (e) {
      update((s) => ({ ...s, error: String(e) }));
    }
  }

  async function stopAll() {
    try {
      await api.stopService('all');
      await refresh();
    } catch (e) {
      update((s) => ({ ...s, error: String(e) }));
    }
  }

  function startPolling() {
    if (pollTimer) return;
    refresh();
    pollTimer = setInterval(refresh, 2000);
    // Also refresh whenever a status event comes in.
    api.onServiceStatusChanged(() => refresh()).then((u) => {
      unlistenStatus = () => u();
    });
  }

  function stopPolling() {
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
    if (unlistenStatus) {
      unlistenStatus();
      unlistenStatus = null;
    }
  }

  return {
    subscribe,
    refresh,
    start,
    stop,
    startAll,
    stopAll,
    startPolling,
    stopPolling
  };
}

export const services = createServicesStore();

// ---- Projects store -------------------------------------------------------

interface ProjectsState {
  list: Project[];
  loading: boolean;
  error: string | null;
}

function createProjectsStore() {
  const { subscribe, set, update } = writable<ProjectsState>({
    list: [],
    loading: false,
    error: null
  });

  async function refresh() {
    update((s) => ({ ...s, loading: true, error: null }));
    try {
      const list = await api.listProjects();
      set({ list, loading: false, error: null });
    } catch (e) {
      update((s) => ({ ...s, loading: false, error: String(e) }));
    }
  }

  async function create(name: string, kind: Project['kind']) {
    try {
      await api.createProject(name, kind);
      await refresh();
    } catch (e) {
      update((s) => ({ ...s, error: String(e) }));
      throw e;
    }
  }

  async function remove(name: string) {
    try {
      await api.deleteProject(name);
      await refresh();
    } catch (e) {
      update((s) => ({ ...s, error: String(e) }));
      throw e;
    }
  }

  return { subscribe, refresh, create, remove };
}

export const projects = createProjectsStore();

// ---- Settings store -------------------------------------------------------

interface SettingsState {
  current: Settings | null;
  loading: boolean;
  error: string | null;
}

function createSettingsStore() {
  const { subscribe, set, update } = writable<SettingsState>({
    current: null,
    loading: false,
    error: null
  });

  async function refresh() {
    update((s) => ({ ...s, loading: true, error: null }));
    try {
      const current = await api.getSettings();
      set({ current, loading: false, error: null });
    } catch (e) {
      update((s) => ({ ...s, loading: false, error: String(e) }));
    }
  }

  async function updateField(key: string, value: unknown) {
    try {
      const current = await api.updateSettings(key, value);
      set({ current, loading: false, error: null });
    } catch (e) {
      update((s) => ({ ...s, error: String(e) }));
    }
  }

  async function reset() {
    try {
      const current = await api.resetSettings();
      set({ current, loading: false, error: null });
    } catch (e) {
      update((s) => ({ ...s, error: String(e) }));
    }
  }

  return { subscribe, refresh, updateField, reset };
}

export const settings = createSettingsStore();

// ---- Binaries store -------------------------------------------------------

interface BinariesState {
  list: BinaryStatus[];
  loading: boolean;
  error: string | null;
}

function createBinariesStore() {
  const { subscribe, set, update } = writable<BinariesState>({
    list: [],
    loading: false,
    error: null
  });

  async function refresh() {
    update((s) => ({ ...s, loading: true, error: null }));
    try {
      const list = await api.listBinaries();
      set({ list, loading: false, error: null });
    } catch (e) {
      update((s) => ({ ...s, loading: false, error: String(e) }));
    }
  }

  async function download(name: string, version: string) {
    try {
      await api.downloadBinary(name, version);
      await refresh();
    } catch (e) {
      update((s) => ({ ...s, error: String(e) }));
    }
  }

  return { subscribe, refresh, download };
}

export const binaries = createBinariesStore();

// ---- Derived helpers ------------------------------------------------------

export const servicesRunning: Readable<boolean> = {
  subscribe(cb) {
    return services.subscribe((s) => cb(s.list.some((x) => x.running)));
  }
};
