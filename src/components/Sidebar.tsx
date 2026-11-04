import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import {
  BookOpen,
  ChevronRight,
  FileInput,
  Folder,
  Layers3,
  Library,
  Plus,
  Settings,
  Star,
  Tag,
  Trash2,
  X,
} from 'lucide-react';
import { useState } from 'react';

import { api, isNative } from '../lib/api';
import { emptyFilter, type ImportProgress } from '../types';
import { useLibraryStore } from '../store/useLibraryStore';

interface Props {
  open: boolean;
  onClose: () => void;
  onOpenNote: () => void;
  onSettings: () => void;
}

export function Sidebar({ open: visible, onClose, onOpenNote, onSettings }: Props) {
  const {
    snapshot,
    filter,
    setFilter,
    selectNote,
    createNote,
    createNotebook,
    createStack,
    createTag,
    toggleShortcut,
    refreshNotes,
    refreshSnapshot,
  } = useLibraryStore();
  const [importing, setImporting] = useState(false);
  const [importStatus, setImportStatus] = useState<string | null>(null);
  const [importProgress, setImportProgress] = useState<ImportProgress | null>(null);
  const [collapsedStacks, setCollapsedStacks] = useState<Record<string, boolean>>({});

  const navigate = async (next: Parameters<typeof setFilter>[0]) => {
    await setFilter({ ...emptyFilter, ...next });
    onClose();
  };
  const newNote = async () => {
    await createNote();
    onOpenNote();
    onClose();
  };
  const importFile = async () => {
    if (!isNative) return;
    const path = await open({
      multiple: false,
      title: 'Import Evernote ENEX',
      filters: [{ name: 'Evernote export', extensions: ['enex'] }],
    });
    if (!path || Array.isArray(path)) return;
    const importId = crypto.randomUUID();
    let active = true;
    let refreshTimer: number | null = null;
    let refreshInFlight: Promise<void> | null = null;
    let lastRefresh = 0;
    let unlisten: (() => void) | undefined;
    const scheduleRefresh = () => {
      if (!active || refreshTimer !== null || refreshInFlight) return;
      refreshTimer = window.setTimeout(
        () => {
          refreshTimer = null;
          if (!active) return;
          lastRefresh = Date.now();
          refreshInFlight = Promise.all([refreshSnapshot(), refreshNotes()])
            .then(() => undefined)
            .finally(() => {
              refreshInFlight = null;
            });
        },
        Math.max(0, 1000 - (Date.now() - lastRefresh)),
      );
    };
    setImporting(true);
    setImportStatus(null);
    setImportProgress(null);
    try {
      unlisten = await listen<ImportProgress>('bevernote:import-progress', (event) => {
        const progress = event.payload;
        if (progress.importId !== importId) return;
        setImportProgress(progress);
        if (progress.phase === 'notebookReady') {
          lastRefresh = Date.now();
          void refreshSnapshot().then(() => setFilter({ ...emptyFilter, notebookId: progress.notebookId }));
        } else if (['reading', 'processing', 'noteImported'].includes(progress.phase)) {
          scheduleRefresh();
        }
      });
      const report = await api.importEnex(path, null, importId);
      active = false;
      if (refreshTimer !== null) window.clearTimeout(refreshTimer);
      await Promise.resolve(refreshInFlight);
      await refreshSnapshot();
      await setFilter({ ...emptyFilter, notebookId: report.notebookId });
      setImportStatus(
        `${report.notesImported} notes and ${report.resourcesImported} attachments imported.${
          report.notesSkipped ? ` ${report.notesSkipped} already present; local edits preserved.` : ''
        }${report.warnings.length ? ` ${report.warnings.join(' ')}` : ''}`,
      );
    } catch (error) {
      setImportStatus(String(error));
    } finally {
      active = false;
      if (refreshTimer !== null) window.clearTimeout(refreshTimer);
      unlisten?.();
      setImporting(false);
    }
  };

  return (
    <>
      {visible && <button className="sidebar-scrim" aria-label="Close navigation" onClick={onClose} />}
      <aside className={`sidebar ${visible ? 'is-open' : ''}`}>
        <div className="brand">
          <img className="brand-mark" src="/bevernote.svg" alt="" />
          <span>Bevernote</span>
          <button className="mobile-close icon-button" onClick={onClose} aria-label="Close">
            <X />
          </button>
        </div>
        <button className="new-note-button" onClick={() => void newNote()}>
          <Plus size={18} />
          New note<span>Ctrl N</span>
        </button>
        <nav className="primary-nav">
          <button
            className={
              !filter.notebookId && !filter.stackId && !filter.tagIds.length && !filter.trashOnly
                ? 'active'
                : ''
            }
            onClick={() => void navigate({})}
          >
            <Library />
            All notes
          </button>
          <button
            disabled={isNative && importing}
            onClick={() => void importFile()}
            title={isNative ? 'Import an Evernote export' : 'Available in the native app'}
          >
            <FileInput />
            {importing ? 'Importing…' : 'Import ENEX'}
          </button>
        </nav>
        {importing && importProgress && (
          <section className="import-progress" aria-label="ENEX import progress" aria-live="polite">
            <div>
              <strong>
                {importProgress.phase === 'finalizing'
                  ? 'Finalizing import…'
                  : importProgress.phase === 'notebookReady'
                    ? 'Notebook ready'
                    : importProgress.phase === 'complete'
                      ? 'Import complete'
                      : 'Importing ENEX…'}
              </strong>
              <span>
                {importProgress.totalBytes
                  ? `${Math.min(100, Math.floor((importProgress.bytesRead / importProgress.totalBytes) * 100))}%`
                  : 'Starting…'}
              </span>
            </div>
            <p title={importProgress.noteTitle ?? undefined}>
              {importProgress.phase === 'finalizing'
                ? 'Finalizing internal links and search index…'
                : (importProgress.noteTitle ??
                  (importProgress.phase === 'notebookReady'
                    ? 'The notebook is ready; notes will appear as they are imported.'
                    : importProgress.noteIndex
                      ? `Reading note ${importProgress.noteIndex}…`
                      : 'Preparing the notebook…'))}
            </p>
            <progress
              aria-label="Import progress"
              max={importProgress.totalBytes || 1}
              value={importProgress.bytesRead}
            />
            <small>
              {importProgress.noteIndex} read · {importProgress.notesImported} imported ·{' '}
              {importProgress.notesSkipped} skipped · {importProgress.resourcesImported} attachments
            </small>
          </section>
        )}
        {importStatus && <p className="import-status">{importStatus}</p>}
        <div className="sidebar-scroll" tabIndex={0} aria-label="Notebook navigation">
          <section className="sidebar-section">
            <h2>
              <Star size={13} />
              Shortcuts
            </h2>
            {snapshot.shortcuts.length === 0 && (
              <p className="sidebar-empty">Star a note or notebook for quick access.</p>
            )}
            {snapshot.shortcuts.map((shortcut) => (
              <button
                className="tree-row"
                key={shortcut.id}
                onClick={() => {
                  if (shortcut.targetType === 'note') {
                    void selectNote(shortcut.targetId);
                    onOpenNote();
                  } else if (shortcut.targetType === 'notebook')
                    void navigate({ notebookId: shortcut.targetId });
                  else if (shortcut.targetType === 'stack') void navigate({ stackId: shortcut.targetId });
                  else if (shortcut.targetType === 'tag') void navigate({ tagIds: [shortcut.targetId] });
                  onClose();
                }}
              >
                <Star size={14} />
                <span>{shortcut.label}</span>
              </button>
            ))}
          </section>
          <section className="sidebar-section">
            <h2>
              <BookOpen size={13} />
              Notebooks{' '}
              <span className="section-actions">
                <button
                  onClick={() => {
                    const name = prompt('Stack name');
                    if (name) void createStack(name);
                  }}
                  title="New stack"
                >
                  <Layers3 />
                </button>
                <button
                  onClick={() => {
                    const name = prompt('Notebook name');
                    if (name) void createNotebook(name);
                  }}
                  title="New notebook"
                >
                  <Plus />
                </button>
              </span>
            </h2>
            {snapshot.notebooks
              .filter((notebook) => !notebook.stackId)
              .map((notebook) => (
                <NotebookRow
                  key={notebook.id}
                  id={notebook.id}
                  name={notebook.name}
                  active={filter.notebookId === notebook.id}
                  onClick={() => void navigate({ notebookId: notebook.id })}
                  onStar={() => void toggleShortcut('notebook', notebook.id, notebook.name)}
                  stacks={snapshot.stacks}
                  stackId={notebook.stackId}
                />
              ))}
            {snapshot.stacks.map((stack) => (
              <div className="stack-group" key={stack.id}>
                <div className={`stack-row ${filter.stackId === stack.id ? 'active' : ''}`}>
                  <button
                    className="stack-toggle"
                    aria-label={`${collapsedStacks[stack.id] ? 'Expand' : 'Collapse'} stack ${stack.name}`}
                    aria-expanded={!collapsedStacks[stack.id]}
                    onClick={() =>
                      setCollapsedStacks((current) => ({ ...current, [stack.id]: !current[stack.id] }))
                    }
                  >
                    <ChevronRight className={collapsedStacks[stack.id] ? '' : 'expanded'} size={13} />
                  </button>
                  <button
                    className="tree-row stack-name"
                    onClick={() => void navigate({ stackId: stack.id })}
                  >
                    <Layers3 size={14} />
                    <span>{stack.name}</span>
                  </button>
                </div>
                {!collapsedStacks[stack.id] && (
                  <div className="stack-children">
                    {snapshot.notebooks
                      .filter((notebook) => notebook.stackId === stack.id)
                      .map((notebook) => (
                        <NotebookRow
                          key={notebook.id}
                          id={notebook.id}
                          name={notebook.name}
                          active={filter.notebookId === notebook.id}
                          onClick={() => void navigate({ notebookId: notebook.id })}
                          onStar={() => void toggleShortcut('notebook', notebook.id, notebook.name)}
                          stacks={snapshot.stacks}
                          stackId={notebook.stackId}
                        />
                      ))}
                  </div>
                )}
              </div>
            ))}
          </section>
          <section className="sidebar-section">
            <h2>
              <Tag size={13} />
              Tags{' '}
              <button
                className="section-add"
                onClick={() => {
                  const name = prompt('Tag name');
                  if (name) void createTag(name);
                }}
                title="New tag"
              >
                <Plus />
              </button>
            </h2>
            <div className="sidebar-tags">
              {snapshot.tags.map((tag) => (
                <button
                  key={tag.id}
                  className={filter.tagIds.includes(tag.id) ? 'active' : ''}
                  onClick={() => void navigate({ tagIds: [tag.id] })}
                >
                  #{tag.name}
                </button>
              ))}
            </div>
          </section>
        </div>
        <div className="sidebar-footer">
          <button
            className={filter.trashOnly ? 'active' : ''}
            onClick={() => void navigate({ trashOnly: true, includeTrash: true })}
          >
            <Trash2 />
            Trash
          </button>
          <button onClick={onSettings}>
            <Settings />
            Settings & sync
          </button>
          <div className="local-status">
            <span className="status-dot" />
            Stored locally
          </div>
        </div>
      </aside>
    </>
  );
}

function NotebookRow({
  id,
  name,
  active,
  onClick,
  onStar,
  stacks,
  stackId,
}: {
  id: string;
  name: string;
  active: boolean;
  onClick: () => void;
  onStar: () => void;
  stacks: { id: string; name: string }[];
  stackId: string | null;
}) {
  const setNotebookStack = useLibraryStore((state) => state.setNotebookStack);
  return (
    <div className={`notebook-row ${active ? 'active' : ''}`}>
      <button className="tree-row" onClick={onClick}>
        <Folder size={14} />
        <span>{name}</span>
      </button>
      <select
        className="notebook-stack-select"
        aria-label={`Move ${name} to stack`}
        title="Move to stack"
        value={stackId ?? ''}
        onChange={(event) => void setNotebookStack(id, event.target.value || null)}
      >
        <option value="">No stack</option>
        {stacks.map((stack) => (
          <option key={stack.id} value={stack.id}>
            {stack.name}
          </option>
        ))}
      </select>
      <button className="row-star" data-notebook={id} aria-label={`Shortcut ${name}`} onClick={onStar}>
        <Star />
      </button>
    </div>
  );
}
