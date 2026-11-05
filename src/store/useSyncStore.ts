import { listen } from '@tauri-apps/api/event';
import { create } from 'zustand';

import { api, isNative } from '../lib/api';
import type { SyncStatus } from '../types';
import { useLibraryStore } from './useLibraryStore';

interface SyncState {
  status: SyncStatus | null;
  starting: boolean;
  initialized: boolean;
  receivedAt: number;
  error: string | null;
  initialize: () => Promise<void>;
  refresh: () => Promise<void>;
  applyStatus: (status: SyncStatus) => void;
  start: () => Promise<SyncStatus | null>;
  clearError: () => void;
}

let listening = false;
let refreshSequence = 0;
let refreshedRunId = 0;

/** A single subscription survives Settings, navigation, and window visibility. */
export const useSyncStore = create<SyncState>((set, get) => {
  const applyStatus = (status: SyncStatus) => {
    const current = get().status;
    // Native revisions are monotonic. A slower status query can never replace
    // a newer event received while Settings was being opened.
    if (current && status.revision < current.revision) return;
    set({ status, receivedAt: Date.now(), error: null });
    if (!status.running && status.lastReport && status.runId !== refreshedRunId) {
      refreshedRunId = status.runId;
      const library = useLibraryStore.getState();
      void Promise.all([library.refreshNotes(), library.refreshSnapshot()]);
    }
  };

  return {
    status: null,
    starting: false,
    initialized: false,
    receivedAt: 0,
    error: null,
    initialize: async () => {
      if (!isNative) {
        set({ initialized: true });
        return;
      }
      if (!listening) {
        listening = true;
        try {
          await listen<SyncStatus>('bevernote:sync-status', (event) => applyStatus(event.payload));
        } catch (error) {
          listening = false;
          set({ error: message(error) });
          return;
        }
      }
      await get().refresh();
      set({ initialized: true });
    },
    refresh: async () => {
      const sequence = ++refreshSequence;
      try {
        const status = await api.syncStatus();
        if (sequence === refreshSequence) applyStatus(status);
      } catch (error) {
        if (sequence === refreshSequence) set({ error: message(error) });
      }
    },
    applyStatus,
    start: async () => {
      if (get().starting || get().status?.running) return get().status;
      set({ starting: true, error: null });
      try {
        const status = await api.syncNow();
        applyStatus(status);
        return status;
      } catch (error) {
        set({ error: message(error) });
        return null;
      } finally {
        set({ starting: false });
      }
    },
    clearError: () => set({ error: null }),
  };
});

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
