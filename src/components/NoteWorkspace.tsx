import { save } from '@tauri-apps/plugin-dialog';
import { ArrowLeft, Check, Download, Info, MoreHorizontal, Plus, Star, Trash2, X } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';

import { RichEditor } from '../editor/RichEditor';
import { api, isNative } from '../lib/api';
import { useLibraryStore } from '../store/useLibraryStore';
import type { Attributes, Note } from '../types';

export function NoteWorkspace({ onBack }: { onBack: () => void }) {
  const {
    selectedNote: note,
    snapshot,
    saveNote,
    toggleShortcut,
    trashSelected,
    restoreSelected,
    deleteSelected,
    attachFile,
    updateNoteTimestamps,
  } = useLibraryStore();
  if (!note) return <EmptyWorkspace />;
  return (
    <NoteEditor
      key={note.id}
      note={note}
      notebooks={snapshot.notebooks}
      shortcuts={snapshot.shortcuts}
      onBack={onBack}
      onSave={saveNote}
      onToggleShortcut={toggleShortcut}
      onTrash={trashSelected}
      onRestore={restoreSelected}
      onDelete={deleteSelected}
      onAttach={attachFile}
      onUpdateTimestamps={updateNoteTimestamps}
    />
  );
}

interface EditorProps {
  note: Note;
  notebooks: ReturnType<typeof useLibraryStore.getState>['snapshot']['notebooks'];
  shortcuts: ReturnType<typeof useLibraryStore.getState>['snapshot']['shortcuts'];
  onBack: () => void;
  onSave: (input: NoteEditorDraft) => Promise<void>;
  onToggleShortcut: (type: string, id: string, label: string) => Promise<void>;
  onTrash: () => Promise<void>;
  onRestore: () => Promise<void>;
  onDelete: () => Promise<void>;
  onAttach: (path: string) => Promise<Note | null>;
  onUpdateTimestamps: (id: string, createdAt: number, updatedAt: number) => Promise<void>;
}

type NoteEditorDraft = Pick<Note, 'id' | 'notebookId' | 'title' | 'contentHtml' | 'attributes' | 'tags'>;

function NoteEditor({
  note,
  notebooks,
  shortcuts,
  onBack,
  onSave,
  onToggleShortcut,
  onTrash,
  onRestore,
  onDelete,
  onAttach,
  onUpdateTimestamps,
}: EditorProps) {
  const [draft, setDraft] = useState<NoteEditorDraft>({
    id: note.id,
    notebookId: note.notebookId,
    title: note.title,
    contentHtml: note.contentHtml,
    attributes: note.attributes,
    tags: note.tags,
  });
  const [dirty, setDirty] = useState(false);
  const [saveState, setSaveState] = useState<'saved' | 'saving' | 'error'>('saved');
  const [metadataOpen, setMetadataOpen] = useState(false);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const [tagInput, setTagInput] = useState('');
  const version = useRef(0);
  const saveInFlight = useRef<Promise<void> | null>(null);
  const isShortcut = shortcuts.some((item) => item.targetType === 'note' && item.targetId === note.id);

  const persistDraft = useCallback(
    (captured: number) => {
      setSaveState('saving');
      const operation = onSave(draft)
        .then(() => {
          if (version.current === captured) {
            setDirty(false);
            setSaveState('saved');
          }
        })
        .catch((error: unknown) => {
          setSaveState('error');
          throw error;
        });
      saveInFlight.current = operation;
      void operation.then(
        () => {
          if (saveInFlight.current === operation) saveInFlight.current = null;
        },
        () => {
          if (saveInFlight.current === operation) saveInFlight.current = null;
        },
      );
      return operation;
    },
    [draft, onSave],
  );

  useEffect(() => {
    if (!dirty || note.deletedAt) return;
    const captured = version.current;
    const timer = window.setTimeout(() => {
      if (captured === version.current) void persistDraft(captured).catch(() => undefined);
    }, 650);
    return () => window.clearTimeout(timer);
  }, [dirty, note.deletedAt, persistDraft]);

  const update = <K extends keyof NoteEditorDraft>(key: K, value: NoteEditorDraft[K]) => {
    version.current += 1;
    setDraft((current) => ({ ...current, [key]: value }));
    setDirty(true);
  };
  const addTag = () => {
    const name = tagInput.trim().replace(/^#/, '');
    if (name && !draft.tags.some((tag) => tag.toLowerCase() === name.toLowerCase()))
      update('tags', [...draft.tags, name]);
    setTagInput('');
  };
  const exportNote = async () => {
    if (!isNative) return;
    const path = await save({
      defaultPath: `${draft.title || 'Untitled'}.enex`,
      filters: [{ name: 'Evernote export', extensions: ['enex'] }],
    });
    if (!path) return;
    if (dirty) await onSave(draft);
    await api.exportEnex([note.id], path);
  };
  const applyTimestamps = async (createdAt: number, updatedAt: number) => {
    const hadUnsavedChanges = dirty;
    const captured = ++version.current;
    setDirty(false);
    try {
      if (saveInFlight.current) await saveInFlight.current;
      if (hadUnsavedChanges) await persistDraft(captured);
      setSaveState('saving');
      await onUpdateTimestamps(note.id, createdAt, updatedAt);
      if (version.current === captured) setSaveState('saved');
    } catch (error) {
      setSaveState('error');
      throw error;
    }
  };

  return (
    <main className="editor-panel">
      <header className="workspace-header">
        <button className="icon-button editor-back" onClick={onBack} aria-label="Back to notes">
          <ArrowLeft />
        </button>
        <div className={`save-status ${saveState}`} aria-live="polite">
          {saveState === 'saving' ? (
            <span className="saving-spinner" />
          ) : saveState === 'saved' ? (
            <Check />
          ) : (
            <X />
          )}
          <span>
            {saveState === 'saving' ? 'Saving…' : saveState === 'saved' ? 'Saved locally' : 'Save failed'}
          </span>
        </div>
        <div className="workspace-actions">
          <button
            className={`icon-button ${isShortcut ? 'is-starred' : ''}`}
            title="Toggle shortcut"
            onClick={() => void onToggleShortcut('note', note.id, draft.title || 'Untitled')}
          >
            <Star />
          </button>
          <button
            className={`icon-button ${metadataOpen ? 'active' : ''}`}
            title="Note information"
            onClick={() => setMetadataOpen((value) => !value)}
          >
            <Info />
          </button>
          <button
            className="icon-button desktop-export"
            title="Export ENEX"
            onClick={() => void exportNote()}
          >
            <Download />
          </button>
          {note.deletedAt ? (
            <>
              <button className="text-action" onClick={() => void onRestore()}>
                Restore
              </button>
              <button
                className="icon-button danger"
                title="Delete permanently"
                onClick={() => {
                  if (confirm('Delete this note permanently?')) void onDelete();
                }}
              >
                <Trash2 />
              </button>
            </>
          ) : (
            <button
              className="icon-button"
              title="Move to trash"
              onClick={() => {
                void onTrash();
                onBack();
              }}
            >
              <Trash2 />
            </button>
          )}
          <button
            className="icon-button"
            title="Keyboard shortcuts"
            aria-label="Keyboard shortcuts"
            onClick={() => setShortcutsOpen(true)}
          >
            <MoreHorizontal />
          </button>
        </div>
      </header>
      <div className={`workspace-body ${metadataOpen ? 'with-metadata' : ''}`}>
        <div className="document-column" tabIndex={0} aria-label="Note document">
          <div className="note-identity">
            <select
              aria-label="Notebook"
              value={draft.notebookId}
              disabled={Boolean(note.deletedAt)}
              onChange={(event) => update('notebookId', event.target.value)}
            >
              {notebooks.map((notebook) => (
                <option key={notebook.id} value={notebook.id}>
                  {notebook.name}
                </option>
              ))}
            </select>
            <span>·</span>
            <time>
              {new Date(note.updatedAt).toLocaleDateString([], {
                month: 'long',
                day: 'numeric',
                year: 'numeric',
              })}
            </time>
          </div>
          <textarea
            className="note-title"
            aria-label="Note title"
            placeholder="Untitled"
            rows={1}
            value={draft.title}
            readOnly={Boolean(note.deletedAt)}
            onInput={(event) => {
              const input = event.currentTarget;
              input.style.height = 'auto';
              input.style.height = `${input.scrollHeight}px`;
            }}
            onChange={(event) => update('title', event.target.value)}
          />
          <div className="note-tags">
            <span>#</span>
            {draft.tags.map((tag) => (
              <button
                key={tag}
                title={`Remove ${tag}`}
                onClick={() =>
                  update(
                    'tags',
                    draft.tags.filter((item) => item !== tag),
                  )
                }
              >
                {tag}
                <X />
              </button>
            ))}
            <input
              aria-label="Add a tag"
              placeholder="Add tag"
              value={tagInput}
              readOnly={Boolean(note.deletedAt)}
              onChange={(event) => setTagInput(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' || event.key === ',') {
                  event.preventDefault();
                  addTag();
                }
              }}
              onBlur={addTag}
            />
            <Plus className="tag-plus" />
          </div>
          {note.deletedAt ? (
            <pre className="trashed-preview">{note.plainText}</pre>
          ) : (
            <RichEditor
              note={{ ...note, ...draft }}
              onChange={(html) => update('contentHtml', html)}
              onAttach={onAttach}
            />
          )}
        </div>
        {metadataOpen && (
          <MetadataPanel
            attributes={draft.attributes}
            createdAt={note.createdAt}
            updatedAt={note.updatedAt}
            originalEnml={note.originalEnml}
            onChange={(attributes) => update('attributes', attributes)}
            onApplyTimestamps={applyTimestamps}
            onClose={() => setMetadataOpen(false)}
          />
        )}
      </div>
      {shortcutsOpen && <KeyboardShortcuts onClose={() => setShortcutsOpen(false)} />}
    </main>
  );
}

function KeyboardShortcuts({ onClose }: { onClose: () => void }) {
  const shortcuts = [
    ['Find in this note', 'Ctrl / Cmd + F'],
    ['Search the whole library', 'Ctrl / Cmd + K or Ctrl / Cmd + Shift + F'],
    ['Create a note', 'Ctrl / Cmd + N'],
    ['Bold, italic, underline', 'Ctrl / Cmd + B, I, U'],
    ['Undo or redo', 'Ctrl / Cmd + Z or Ctrl / Cmd + Shift + Z'],
    ['Close find, dialog, or navigation', 'Escape'],
  ];
  return (
    <div className="modal-backdrop" onMouseDown={onClose}>
      <section
        className="modal shortcuts-modal"
        role="dialog"
        aria-modal="true"
        aria-label="Keyboard shortcuts"
        onMouseDown={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          if (event.key === 'Escape') {
            event.stopPropagation();
            onClose();
          }
        }}
      >
        <header>
          <div>
            <p className="eyebrow">Contextual actions</p>
            <h2>Keyboard shortcuts</h2>
          </div>
          <button className="icon-button" autoFocus onClick={onClose} aria-label="Close keyboard shortcuts">
            <X />
          </button>
        </header>
        <dl>
          {shortcuts.map(([action, keys]) => (
            <div key={action}>
              <dt>{action}</dt>
              <dd>{keys}</dd>
            </div>
          ))}
        </dl>
      </section>
    </div>
  );
}

function MetadataPanel({
  attributes,
  createdAt,
  updatedAt,
  originalEnml,
  onChange,
  onApplyTimestamps,
  onClose,
}: {
  attributes: Attributes;
  createdAt: number;
  updatedAt: number;
  originalEnml: string | null;
  onChange: (value: Attributes) => void;
  onApplyTimestamps: (createdAt: number, updatedAt: number) => Promise<void>;
  onClose: () => void;
}) {
  const set = (key: string, value: string) => {
    const next = { ...attributes };
    if (value) next[key] = value;
    else delete next[key];
    onChange(next);
  };
  return (
    <aside className="metadata-panel">
      <header>
        <div>
          <p className="eyebrow">Details</p>
          <h2>Note information</h2>
        </div>
        <button className="icon-button" onClick={onClose} aria-label="Close note information">
          <X />
        </button>
      </header>
      <TimestampEditor
        key={`${createdAt}-${updatedAt}`}
        createdAt={createdAt}
        updatedAt={updatedAt}
        onApply={onApplyTimestamps}
      />
      {[
        ['author', 'Author'],
        ['source', 'Source'],
        ['source-url', 'Source URL'],
        ['subject-date', 'Subject date'],
        ['place-name', 'Place'],
        ['latitude', 'Latitude'],
        ['longitude', 'Longitude'],
      ].map(([key, label]) => (
        <label key={key}>
          {label}
          <input
            value={typeof attributes[key!] === 'string' ? (attributes[key!] as string) : ''}
            placeholder={key === 'source-url' ? 'https://…' : ''}
            onChange={(event) => set(key!, event.target.value)}
          />
        </label>
      ))}
      {originalEnml && (
        <div className="source-preservation">
          <Check />
          <p>
            <strong>Original ENML preserved</strong>
            <span>The imported source remains alongside your editable note.</span>
          </p>
        </div>
      )}
    </aside>
  );
}

function TimestampEditor({
  createdAt,
  updatedAt,
  onApply,
}: {
  createdAt: number;
  updatedAt: number;
  onApply: (createdAt: number, updatedAt: number) => Promise<void>;
}) {
  const [created, setCreated] = useState(() => localDateTimeValue(createdAt));
  const [updated, setUpdated] = useState(() => localDateTimeValue(updatedAt));
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const apply = async () => {
    const createdTimestamp = new Date(created).getTime();
    const updatedTimestamp = new Date(updated).getTime();
    setMessage(null);
    if (!Number.isFinite(createdTimestamp) || !Number.isFinite(updatedTimestamp)) {
      setError('Enter a valid created and updated date and time.');
      return;
    }
    if (createdTimestamp < 0 || updatedTimestamp < createdTimestamp) {
      setError('Updated time must be at or after created time.');
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await onApply(createdTimestamp, updatedTimestamp);
      setMessage('Timestamps updated.');
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  };

  return (
    <form
      className="timestamp-editor"
      onSubmit={(event) => {
        event.preventDefault();
        void apply();
      }}
    >
      <label>
        Created
        <input
          aria-label="Created date and time"
          type="datetime-local"
          step={1}
          required
          disabled={saving}
          value={created}
          onChange={(event) => {
            setCreated(event.target.value);
            setError(null);
            setMessage(null);
          }}
        />
      </label>
      <label>
        Updated
        <input
          aria-label="Updated date and time"
          type="datetime-local"
          step={1}
          required
          disabled={saving}
          value={updated}
          onChange={(event) => {
            setUpdated(event.target.value);
            setError(null);
            setMessage(null);
          }}
        />
      </label>
      <button className="primary-action" type="submit" disabled={saving || !created || !updated}>
        {saving ? 'Applying…' : 'Apply timestamps'}
      </button>
      <p>Dates and times use this device’s timezone.</p>
      {error && (
        <p className="timestamp-error" role="alert">
          {error}
        </p>
      )}
      {message && <p role="status">{message}</p>}
    </form>
  );
}

function localDateTimeValue(timestamp: number) {
  const date = new Date(timestamp);
  return new Date(date.getTime() - date.getTimezoneOffset() * 60_000).toISOString().slice(0, 19);
}

function EmptyWorkspace() {
  return (
    <main className="editor-panel empty-workspace">
      <div className="empty-illustration">
        <img src="/bevernote.svg" alt="" />
        <i />
        <i />
        <i />
      </div>
      <h2>A quiet place for your thoughts</h2>
      <p>Select a note to start reading, or create a new one.</p>
      <kbd>Ctrl N</kbd>
    </main>
  );
}
