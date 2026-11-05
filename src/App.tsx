import { listen } from '@tauri-apps/api/event';
import { Check, Cloud, CloudCheck, HardDrive, RefreshCw, ScanText, ShieldCheck, X } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';

import { NoteList } from './components/NoteList';
import { NoteWorkspace } from './components/NoteWorkspace';
import { Sidebar } from './components/Sidebar';
import { api, isNative } from './lib/api';
import { formatBytes, formatDuration } from './lib/syncProgress';
import { useDrawerGestures } from './lib/useDrawerGestures';
import { useLibraryStore } from './store/useLibraryStore';
import { useSyncStore } from './store/useSyncStore';
import type { PCloudStatus } from './types';

const isAndroid = isNative && /Android/i.test(navigator.userAgent);
type AppLayer = 'sidebar' | 'editor' | 'settings';
const isMobileLayout = () => window.matchMedia('(max-width: 760px)').matches;
interface BevernoteHistoryState {
  bevernote?: AppLayer;
}
const historyLayer = () => (window.history.state as BevernoteHistoryState | null)?.bevernote ?? null;

export function App() {
  const { initialize, createNote, error, clearError, loading, selectedNote } = useLibraryStore();
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [mobileEditor, setMobileEditor] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [theme, setTheme] = useState(() => localStorage.getItem('bevernote-theme') ?? 'light');
  const syncStatus = useSyncStore((state) => state.status);
  const syncStarting = useSyncStore((state) => state.starting);

  const setLayer = useCallback((layer: AppLayer | null) => {
    setSidebarOpen(layer === 'sidebar');
    setMobileEditor(layer === 'editor');
    setSettingsOpen(layer === 'settings');
  }, []);
  const openLayer = useCallback(
    (layer: AppLayer) => {
      if (isMobileLayout()) {
        const state = { ...(window.history.state as BevernoteHistoryState | null), bevernote: layer };
        const replaceDrawer = layer !== 'sidebar' && historyLayer() === 'sidebar';
        if (historyLayer() !== layer) {
          window.history[replaceDrawer ? 'replaceState' : 'pushState'](state, '');
        }
      }
      setLayer(layer);
    },
    [setLayer],
  );
  const closeLayer = useCallback(
    (layer: AppLayer) => {
      if (isMobileLayout() && historyLayer() === layer) window.history.back();
      else setLayer(null);
    },
    [setLayer],
  );

  useEffect(() => {
    const onHistory = (event: PopStateEvent) =>
      setLayer((event.state as BevernoteHistoryState | null)?.bevernote ?? null);
    window.addEventListener('popstate', onHistory);
    return () => window.removeEventListener('popstate', onHistory);
  }, [setLayer]);
  useEffect(() => {
    void initialize();
    void useSyncStore.getState().initialize();
  }, [initialize]);
  useEffect(() => {
    // Android applies native system-bar padding in MainActivity. Other shells
    // use CSS safe-area insets; avoid applying both on Android.
    document.documentElement.dataset.platform = isAndroid ? 'android' : 'other';
  }, []);
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem('bevernote-theme', theme);
  }, [theme]);
  useEffect(() => {
    const handle = (event: KeyboardEvent) => {
      const modifier = event.ctrlKey || event.metaKey;
      const key = event.key.toLowerCase();
      if (modifier && key === 'n') {
        event.preventDefault();
        void createNote().then(() => openLayer('editor'));
      }
      if (modifier && (key === 'k' || (key === 'f' && event.shiftKey))) {
        event.preventDefault();
        closeLayer('editor');
        document.querySelector<HTMLInputElement>('.search-wrap input')?.focus();
      }
      if (modifier && key === 'f' && !event.shiftKey && selectedNote) {
        event.preventDefault();
        window.dispatchEvent(new Event('bevernote:find'));
      }
      if (event.key === 'Escape') {
        if (settingsOpen) closeLayer('settings');
        else if (sidebarOpen) closeLayer('sidebar');
      }
    };
    window.addEventListener('keydown', handle);
    return () => window.removeEventListener('keydown', handle);
  }, [createNote, selectedNote, settingsOpen, sidebarOpen, closeLayer, openLayer]);

  const gestures = useDrawerGestures({
    open: sidebarOpen,
    blocked: settingsOpen || (mobileEditor && Boolean(selectedNote)),
    onOpen: () => openLayer('sidebar'),
    onClose: () => closeLayer('sidebar'),
  });
  const syncActive = syncStarting || Boolean(syncStatus?.running);

  return (
    <div className={`app-shell ${mobileEditor && selectedNote ? 'mobile-has-note' : ''}`} {...gestures}>
      <Sidebar
        open={sidebarOpen}
        onClose={() => closeLayer('sidebar')}
        onOpenNote={() => openLayer('editor')}
        onSettings={() => openLayer('settings')}
      />
      <NoteList onOpenSidebar={() => openLayer('sidebar')} onOpenNote={() => openLayer('editor')} />
      <NoteWorkspace onBack={() => closeLayer('editor')} />
      {syncActive && (
        <button
          className="global-sync-indicator"
          onClick={() => openLayer('settings')}
          title="View sync progress"
        >
          <RefreshCw className="spin" />
          <span>{syncStatus?.progress.message ?? 'Preparing synchronization…'}</span>
        </button>
      )}
      {loading && (
        <div className="loading-screen">
          <img className="brand-mark" src="/bevernote.svg" alt="" />
          <strong>Bevernote</strong>
          <span className="saving-spinner" />
        </div>
      )}
      {error && (
        <div className="error-toast" role="alert">
          <span>{error}</span>
          <button onClick={clearError} aria-label="Dismiss">
            <X />
          </button>
        </div>
      )}
      {settingsOpen && (
        <SettingsDialog theme={theme} onTheme={setTheme} onClose={() => closeLayer('settings')} />
      )}
    </div>
  );
}

function SettingsDialog({
  theme,
  onTheme,
  onClose,
}: {
  theme: string;
  onTheme: (theme: string) => void;
  onClose: () => void;
}) {
  const {
    status: syncStatus,
    starting: syncStarting,
    receivedAt: progressReceivedAt,
    error: syncError,
    start: startSync,
  } = useSyncStore();
  const [status, setStatus] = useState<PCloudStatus | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const [oauthPending, setOauthPending] = useState(false);
  const [manualOpen, setManualOpen] = useState(false);
  const [token, setToken] = useState('');
  const [hostname, setHostname] = useState('api.pcloud.com');
  const [ocrEnabled, setOcrEnabled] = useState(true);
  const syncRunning = syncStarting || Boolean(syncStatus?.running);
  const syncProgress = syncStatus?.progress ?? null;
  const report = syncStatus?.lastReport ?? null;

  useEffect(() => {
    void useSyncStore.getState().refresh();
    void api
      .pcloudStatus()
      .then((current) => {
        setStatus(current);
        setOauthPending(current.authorizationPending);
      })
      .catch((error: unknown) => setMessage(String(error)));
    if (!isNative) return;
    void api
      .ocrEnabled()
      .then(setOcrEnabled)
      .catch((error: unknown) => setMessage(String(error)));
  }, []);

  useEffect(() => {
    if (!isNative) return;
    let active = true;
    const subscriptions = [
      listen<PCloudStatus>('bevernote:pcloud-status', (event) => {
        if (!active) return;
        setStatus(event.payload);
        setOauthPending(event.payload.authorizationPending);
        setToken('');
        setManualOpen(false);
        setMessage(null);
      }),
      listen<string>('bevernote:pcloud-error', (event) => {
        if (!active) return;
        setOauthPending(false);
        setMessage(event.payload);
      }),
    ];
    return () => {
      active = false;
      void Promise.all(subscriptions).then((unlisteners) => {
        unlisteners.forEach((unlisten) => unlisten());
      });
    };
  }, []);

  useEffect(() => {
    if (!syncRunning) return;
    const timer = window.setInterval(() => setNow(Date.now()), 500);
    return () => window.clearInterval(timer);
  }, [syncRunning]);

  const run = async (action: string, operation: () => Promise<PCloudStatus>) => {
    setBusy(action);
    setMessage(null);
    if (action === 'connect' && isAndroid) setOauthPending(true);
    try {
      const result = await operation();
      setStatus(result);
      setOauthPending(result.authorizationPending);
      setToken('');
      setManualOpen(false);
      if (action === 'connect' && isAndroid && result.authorizationPending) {
        setMessage('Approve pCloud in your browser. Android will return to Bevernote automatically.');
      }
    } catch (error) {
      if (action === 'connect') setOauthPending(false);
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(null);
    }
  };

  const elapsedMs = syncProgress
    ? syncProgress.elapsedMs + (syncRunning ? Math.max(0, now - progressReceivedAt) : 0)
    : 0;

  return (
    <div className="modal-backdrop" onMouseDown={onClose}>
      <section
        className="modal settings-modal"
        role="dialog"
        aria-modal="true"
        aria-label="Settings"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header>
          <div>
            <p className="eyebrow">Preferences</p>
            <h2>Settings</h2>
          </div>
          <button className="icon-button" onClick={onClose} aria-label="Close settings">
            <X />
          </button>
        </header>
        <div className="settings-content">
          <section className="settings-card">
            <HardDrive />
            <div>
              <h3>Local-first library</h3>
              <p>
                Your notes, search index, and attachments stay on this device. Bevernote has no server-side
                compute.
              </p>
              <span className="setting-ok">
                <Check />
                Offline and ready
              </span>
            </div>
          </section>
          <section className="settings-card">
            <ScanText />
            <div>
              <h3>Offline image OCR</h3>
              <p>
                Index text in PNG, JPEG, and WebP images during ENEX import. Best for Latin-alphabet text;
                original images are never changed and recognition failures do not stop import.
              </p>
              <label className="ocr-setting">
                <input
                  type="checkbox"
                  checked={ocrEnabled}
                  disabled={!isNative}
                  onChange={(event) => {
                    const enabled = event.target.checked;
                    setOcrEnabled(enabled);
                    void api.setOcrEnabled(enabled).catch((error: unknown) => {
                      setOcrEnabled(!enabled);
                      setMessage(String(error));
                    });
                  }}
                />
                Recognize image text on this device
              </label>
              {!isNative && <span className="settings-pending">Available in the native app.</span>}
            </div>
          </section>
          <section className="settings-card">
            <ShieldCheck />
            <div>
              <h3>Your data remains portable</h3>
              <p>
                Import and export standard Evernote ENEX files. Original ENML and attachment metadata are
                preserved.
              </p>
            </div>
          </section>
          <section className="settings-card">
            <Cloud />
            <div className="sync-settings">
              <h3>Optional pCloud sync</h3>
              <p>
                Synchronize notes and attachments directly with your pCloud storage. HTTPS protects data in
                transit; Bevernote does not encrypt the remote copy.
              </p>
              {!status?.connected && (
                <span className="settings-pending">
                  {status === null
                    ? 'Checking pCloud connection…'
                    : oauthPending
                      ? 'Waiting for pCloud authorization in your browser…'
                      : 'Connect pCloud to enable sync.'}
                </span>
              )}
              <div className="settings-buttons sync-primary-actions">
                <button
                  className="primary-action"
                  disabled={!isNative || !status?.connected || busy !== null || syncRunning}
                  title={status?.connected ? 'Synchronize with pCloud now' : 'Connect pCloud to enable sync'}
                  onClick={() => void startSync()}
                >
                  <RefreshCw className={syncRunning ? 'spin' : ''} />
                  {syncRunning ? 'Synchronizing…' : 'Sync now'}
                </button>
                {status?.connected && (
                  <button
                    className="secondary-action"
                    disabled={busy !== null || syncRunning}
                    onClick={() => void run('disconnect', api.disconnectPcloud)}
                  >
                    Disconnect
                  </button>
                )}
              </div>
              <span className="settings-pending">
                One sync runs at a time. Progress remains available after closing Settings. On Windows,
                closing the window keeps the current sync in the tray; on Android, the foreground service
                shows a notification when permission allows. Force-stop, reboot, or OS process termination can
                interrupt background completion.
              </span>
              {status?.connected ? (
                <>
                  <span className="setting-ok">
                    <CloudCheck />
                    Connected{status.email ? ` as ${status.email}` : ''} · {status.hostname}
                  </span>
                  {status.lastSyncAt && (
                    <span className="settings-pending">
                      Last synchronized {new Date(status.lastSyncAt).toLocaleString()}
                    </span>
                  )}
                </>
              ) : (
                <>
                  <div className="oauth-note">
                    <p>One-time pCloud app setup:</p>
                    <ol className="oauth-steps">
                      <li>
                        Set <strong>Allow implicit grant</strong> to <strong>Allow</strong>, then click{' '}
                        <strong>Save changes</strong>.
                      </li>
                      {!isAndroid && (
                        <li>
                          Keep the redirect URI exactly <code>http://127.0.0.1:53682/</code>.
                        </li>
                      )}
                      <li>
                        Connect below and approve in the browser. pCloud returns the token and region
                        automatically.
                      </li>
                    </ol>
                  </div>
                  <div className="settings-buttons">
                    <button
                      className="primary-action"
                      disabled={!isNative || busy !== null}
                      onClick={() => void run('connect', api.connectPcloud)}
                    >
                      {busy === 'connect'
                        ? 'Opening browser…'
                        : oauthPending
                          ? 'Restart browser connection'
                          : 'Connect with browser'}
                    </button>
                    <button
                      className="secondary-action"
                      disabled={!isNative || busy !== null}
                      onClick={() => setManualOpen((value) => !value)}
                    >
                      Use access token
                    </button>
                  </div>
                  {manualOpen && (
                    <form
                      className="token-form"
                      onSubmit={(event) => {
                        event.preventDefault();
                        void run('token', () => api.connectPcloudToken(token, hostname));
                      }}
                    >
                      <label>
                        pCloud region
                        <select value={hostname} onChange={(event) => setHostname(event.target.value)}>
                          <option value="api.pcloud.com">Global (api.pcloud.com)</option>
                          <option value="eapi.pcloud.com">Europe (eapi.pcloud.com)</option>
                        </select>
                      </label>
                      <p className="settings-pending">
                        Fallback for a token pCloud already issued to you. Bevernote does not create tokens;
                        browser connection is the recommended way to obtain one securely.
                      </p>
                      <label>
                        Access token
                        <input
                          required
                          type="password"
                          autoComplete="off"
                          value={token}
                          onChange={(event) => setToken(event.target.value)}
                          placeholder="Token is stored in your system credential vault"
                        />
                      </label>
                      <button
                        className="primary-action"
                        type="submit"
                        disabled={!token.trim() || busy !== null}
                      >
                        {busy === 'token' ? 'Connecting…' : 'Connect token'}
                      </button>
                    </form>
                  )}
                </>
              )}
              {!isNative && (
                <span className="settings-pending">pCloud connection is available in the native app.</span>
              )}
              {syncRunning && (
                <div className="sync-progress" role="status" aria-live="polite">
                  <strong>{syncProgress?.message ?? 'Preparing synchronization…'}</strong>
                  {syncProgress && (
                    <>
                      {syncProgress.total > 0 && (
                        <progress
                          aria-label="Current sync operation"
                          max={syncProgress.total}
                          value={Math.min(syncProgress.current, syncProgress.total)}
                        />
                      )}
                      <div className="sync-progress-timing">
                        <span>Elapsed {formatDuration(elapsedMs)}</span>
                        <span>Network {formatDuration(syncProgress.networkMs)}</span>
                      </div>
                      <span className="settings-pending">
                        {syncProgress.remoteRequests} pCloud requests · ↓{' '}
                        {formatBytes(syncProgress.bytesDownloaded)} · ↑{' '}
                        {formatBytes(syncProgress.bytesUploaded)}
                      </span>
                    </>
                  )}
                </div>
              )}
              {syncError && (
                <p className="sync-message error" role="alert">
                  {syncError}
                </p>
              )}
              {syncStatus?.error && !syncError && (
                <p className="sync-message error" role="alert">
                  {syncStatus.error}
                </p>
              )}
              {message && (
                <p
                  className={`sync-message ${oauthPending ? '' : 'error'}`}
                  role={oauthPending ? 'status' : 'alert'}
                >
                  {message}
                </p>
              )}
              {report && (
                <div className="sync-message success" role="status">
                  <strong>Sync complete in {formatDuration(report.durationMs)}</strong>
                  <p>
                    {report.pushed} notes uploaded, {report.pulled} downloaded, {report.conflicts} conflict
                    copies, {report.blobsUploaded + report.blobsDownloaded} attachments transferred.
                  </p>
                  <span className="settings-pending">
                    Network {formatDuration(report.networkMs)} · {report.remoteRequests} requests · ↓{' '}
                    {formatBytes(report.bytesDownloaded)} · ↑ {formatBytes(report.bytesUploaded)}
                  </span>
                </div>
              )}
            </div>
          </section>
          <label className="theme-setting">
            Appearance
            <select value={theme} onChange={(event) => onTheme(event.target.value)}>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
              <option value="system">System</option>
            </select>
          </label>
        </div>
      </section>
    </div>
  );
}
