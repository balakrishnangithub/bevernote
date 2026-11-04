import { listen } from '@tauri-apps/api/event';
import { Check, Cloud, CloudCheck, HardDrive, RefreshCw, ScanText, ShieldCheck, X } from 'lucide-react';
import { useEffect, useState } from 'react';

import { NoteList } from './components/NoteList';
import { NoteWorkspace } from './components/NoteWorkspace';
import { Sidebar } from './components/Sidebar';
import { api, isNative } from './lib/api';
import { useLibraryStore } from './store/useLibraryStore';
import type { PCloudStatus, SyncReport } from './types';

export function App() {
  const { initialize, createNote, error, clearError, loading, selectedNote } = useLibraryStore();
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [mobileEditor, setMobileEditor] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [theme, setTheme] = useState(() => localStorage.getItem('bevernote-theme') ?? 'light');

  useEffect(() => {
    void initialize();
  }, [initialize]);
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
        void createNote().then(() => setMobileEditor(true));
      }
      if (modifier && (key === 'k' || (key === 'f' && event.shiftKey))) {
        event.preventDefault();
        setMobileEditor(false);
        document.querySelector<HTMLInputElement>('.search-wrap input')?.focus();
      }
      if (modifier && key === 'f' && !event.shiftKey && selectedNote) {
        event.preventDefault();
        window.dispatchEvent(new Event('bevernote:find'));
      }
      if (event.key === 'Escape') {
        setSettingsOpen(false);
        setSidebarOpen(false);
      }
    };
    window.addEventListener('keydown', handle);
    return () => window.removeEventListener('keydown', handle);
  }, [createNote, selectedNote]);

  return (
    <div className={`app-shell ${mobileEditor && selectedNote ? 'mobile-has-note' : ''}`}>
      <Sidebar
        open={sidebarOpen}
        onClose={() => setSidebarOpen(false)}
        onOpenNote={() => setMobileEditor(true)}
        onSettings={() => setSettingsOpen(true)}
      />
      <NoteList onOpenSidebar={() => setSidebarOpen(true)} onOpenNote={() => setMobileEditor(true)} />
      <NoteWorkspace onBack={() => setMobileEditor(false)} />
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
        <SettingsDialog theme={theme} onTheme={setTheme} onClose={() => setSettingsOpen(false)} />
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
  const { refreshNotes, refreshSnapshot } = useLibraryStore();
  const [status, setStatus] = useState<PCloudStatus | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [report, setReport] = useState<SyncReport | null>(null);
  const [manualOpen, setManualOpen] = useState(false);
  const [token, setToken] = useState('');
  const [hostname, setHostname] = useState('api.pcloud.com');
  const [ocrEnabled, setOcrEnabled] = useState(true);

  useEffect(() => {
    void api
      .pcloudStatus()
      .then(setStatus)
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
    const subscription = listen<PCloudStatus>('bevernote:pcloud-status', (event) => {
      if (!active) return;
      setStatus(event.payload);
      setToken('');
      setManualOpen(false);
      setMessage(null);
    });
    return () => {
      active = false;
      void subscription.then((unlisten) => unlisten());
    };
  }, []);

  const run = async (action: string, operation: () => Promise<PCloudStatus | SyncReport>) => {
    setBusy(action);
    setMessage(null);
    setReport(null);
    try {
      const result = await operation();
      if ('connected' in result) {
        setStatus(result);
        setToken('');
        setManualOpen(false);
      } else {
        setReport(result);
        setStatus(await api.pcloudStatus());
        await Promise.all([refreshNotes(), refreshSnapshot()]);
      }
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(null);
    }
  };

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
                  {status === null ? 'Checking pCloud connection…' : 'Connect pCloud to enable sync.'}
                </span>
              )}
              <div className="settings-buttons sync-primary-actions">
                <button
                  className="primary-action"
                  disabled={!isNative || !status?.connected || busy !== null}
                  title={status?.connected ? 'Synchronize with pCloud now' : 'Connect pCloud to enable sync'}
                  onClick={() => void run('sync', api.syncNow)}
                >
                  <RefreshCw className={busy === 'sync' ? 'spin' : ''} />
                  {busy === 'sync' ? 'Synchronizing…' : 'Sync now'}
                </button>
                {status?.connected && (
                  <button
                    className="secondary-action"
                    disabled={busy !== null}
                    onClick={() => void run('disconnect', api.disconnectPcloud)}
                  >
                    Disconnect
                  </button>
                )}
              </div>
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
                      <li>
                        Keep the redirect URI exactly <code>http://127.0.0.1:53682/</code>.
                      </li>
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
                      {busy === 'connect' ? 'Waiting for pCloud…' : 'Connect with browser'}
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
              {message && (
                <p className="sync-message error" role="alert">
                  {message}
                </p>
              )}
              {report && (
                <p className="sync-message success" role="status">
                  Sync complete: {report.pushed} uploaded, {report.pulled} downloaded, {report.conflicts}{' '}
                  conflict copies, {report.blobsUploaded + report.blobsDownloaded} attachments transferred.
                </p>
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
