import { beforeEach, describe, expect, it, vi } from 'vitest';

import { api } from '../lib/api';
import type { SyncStatus } from '../types';
import { useLibraryStore } from './useLibraryStore';
import { useSyncStore } from './useSyncStore';

vi.mock('../lib/api', () => ({
  isNative: false,
  api: {
    syncNow: vi.fn(),
    syncStatus: vi.fn(),
  },
}));

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

function status(revision: number, running = true): SyncStatus {
  return {
    revision,
    runId: 1,
    running,
    progress: {
      phase: running ? 'pushing' : 'complete',
      message: running ? `Uploading revision ${revision}` : 'Synchronization complete',
      current: revision,
      total: 20,
      elapsedMs: revision * 100,
      networkMs: 30,
      remoteRequests: revision,
      bytesUploaded: 100,
      bytesDownloaded: 50,
    },
    lastReport: running
      ? null
      : {
          pushed: 2,
          pulled: 1,
          conflicts: 0,
          blobsUploaded: 0,
          blobsDownloaded: 0,
          syncedAt: 123,
          durationMs: 500,
          networkMs: 300,
          remoteRequests: 5,
          bytesUploaded: 100,
          bytesDownloaded: 50,
        },
    error: null,
  };
}

describe('app-wide sync state', () => {
  beforeEach(() => {
    useSyncStore.setState({ status: null, starting: false, initialized: false, receivedAt: 0, error: null });
    vi.clearAllMocks();
  });

  it('guards rapid duplicate clicks before the native call resolves', async () => {
    let resolve: ((value: SyncStatus) => void) | undefined;
    vi.mocked(api.syncNow).mockImplementation(
      () =>
        new Promise<SyncStatus>((done) => {
          resolve = done;
        }),
    );
    const first = useSyncStore.getState().start();
    const duplicate = useSyncStore.getState().start();
    expect(api.syncNow).toHaveBeenCalledTimes(1);
    resolve?.(status(1));
    expect(await first).toMatchObject({ runId: 1, running: true });
    expect(await duplicate).toBeNull();
  });

  it('restores live progress after Settings reopens without accepting stale queries', async () => {
    useSyncStore.getState().applyStatus(status(4));
    vi.mocked(api.syncStatus).mockResolvedValue(status(3));
    await useSyncStore.getState().refresh();
    expect(useSyncStore.getState().status?.progress.message).toBe('Uploading revision 4');
    vi.mocked(api.syncStatus).mockResolvedValue(status(5));
    await useSyncStore.getState().refresh();
    expect(useSyncStore.getState().status?.progress.message).toBe('Uploading revision 5');
  });

  it('keeps terminal reports and refreshes the library once', async () => {
    const refreshNotes = vi.spyOn(useLibraryStore.getState(), 'refreshNotes').mockResolvedValue();
    const refreshSnapshot = vi.spyOn(useLibraryStore.getState(), 'refreshSnapshot').mockResolvedValue();
    useSyncStore.getState().applyStatus(status(2, false));
    expect(useSyncStore.getState().status?.lastReport?.pushed).toBe(2);
    await vi.waitFor(() => expect(refreshNotes).toHaveBeenCalledTimes(1));
    expect(refreshSnapshot).toHaveBeenCalledTimes(1);
    useSyncStore.getState().applyStatus(status(2, false));
    expect(refreshNotes).toHaveBeenCalledTimes(1);
  });
});
