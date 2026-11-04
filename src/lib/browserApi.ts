import type {
  ImportReport,
  LibrarySnapshot,
  Note,
  NoteFilter,
  NoteOrganizationAction,
  NoteSummary,
  SaveNote,
  SyncReport,
} from '../types';
import type { BevernoteApi } from './api';

const now = Date.now();
const inboxId = crypto.randomUUID();
const notes = new Map<string, Note>();
const snapshot: LibrarySnapshot = {
  stacks: [{ id: 'stack-learning', name: 'Learning', createdAt: now, updatedAt: now, revision: 1 }],
  notebooks: [
    { id: inboxId, name: 'Inbox', stackId: null, createdAt: now, updatedAt: now, revision: 1 },
    {
      id: 'notebook-reading',
      name: 'Reading',
      stackId: 'stack-learning',
      createdAt: now,
      updatedAt: now,
      revision: 1,
    },
  ],
  tags: [
    { id: 'tag-local', name: 'local-first', parentId: null, color: null, createdAt: now, updatedAt: now },
    { id: 'tag-ideas', name: 'ideas', parentId: null, color: null, createdAt: now, updatedAt: now },
  ],
  shortcuts: [],
};

function addDemo(note: Note) {
  notes.set(note.id, note);
}

addDemo({
  id: 'welcome',
  notebookId: inboxId,
  title: 'Welcome to Bevernote',
  contentHtml:
    '<h1>Your notes, fully yours.</h1><p>Bevernote is <strong>local-first</strong>, open source, and works completely offline. Your library is stored on your device—not on a server.</p><blockquote><p>Try the formatting tools, create a checklist, or attach a file.</p></blockquote><ul data-type="taskList"><li data-type="taskItem" data-checked="true">Private by default</li><li data-type="taskItem" data-checked="false">Make this note your own</li></ul><p>Search supports <code>intitle:</code>, <code>tag:</code>, <code>notebook:</code>, phrases, prefixes, and negation.</p>',
  originalEnml: null,
  plainText: 'Your notes, fully yours. Bevernote is local-first, open source, and works completely offline.',
  createdAt: now - 100000,
  updatedAt: now - 100000,
  deletedAt: null,
  attributes: { author: 'Bevernote' },
  evernoteGuid: null,
  revision: 1,
  tags: ['local-first', 'ideas'],
  resources: [],
});
addDemo({
  id: 'enex-guide',
  notebookId: 'notebook-reading',
  title: 'Bring your Evernote library',
  contentHtml:
    '<h2>Lossless by design</h2><p>Import an <strong>.enex</strong> file from the sidebar. Bevernote keeps the original ENML, note metadata, attachments, and OCR text while making the note editable.</p><hr><p>Use <mark>ENEX export</mark> whenever you want a portable backup.</p>',
  originalEnml: null,
  plainText: 'Bring your Evernote library. Lossless by design. Import an ENEX file.',
  createdAt: now - 200000,
  updatedAt: now - 200000,
  deletedAt: null,
  attributes: {},
  evernoteGuid: null,
  tags: ['ideas'],
  resources: [],
  revision: 1,
});
addDemo({
  id: 'pdf-preview-demo',
  notebookId: 'notebook-reading',
  title: 'Local attachment preview',
  contentHtml:
    '<h2>Attachments stay usable offline</h2><p>Right-click any attachment for its actions, or preview this public two-page PDF locally.</p><p><en-media hash="demo-pdf" type="application/pdf" title="Bevernote preview.pdf"></en-media></p>',
  originalEnml: null,
  plainText: 'Local attachment preview. Attachments stay usable offline.',
  createdAt: now - 300000,
  updatedAt: now - 300000,
  deletedAt: null,
  attributes: {},
  evernoteGuid: null,
  tags: ['ideas'],
  resources: [
    {
      id: 'demo-pdf-resource',
      noteId: 'pdf-preview-demo',
      sha256: 'demo-pdf',
      md5: 'demo-pdf',
      mime: 'application/pdf',
      fileName: 'Bevernote preview.pdf',
      size: 1800,
      width: null,
      height: null,
      duration: null,
      recognitionXml: null,
      recognitionText: null,
      attributes: { 'file-name': 'Bevernote preview.pdf', attachment: true },
      alternateSha256: null,
      position: 0,
    },
  ],
  revision: 1,
});

function textOf(html: string) {
  const document = new DOMParser().parseFromString(html, 'text/html');
  return document.body.textContent?.replace(/\s+/g, ' ').trim() ?? '';
}

function summaries(filter: NoteFilter): NoteSummary[] {
  const query = filter.query
    .toLowerCase()
    .replace(/^(intitle|tag|notebook):/, '')
    .replaceAll('"', '');
  return [...notes.values()]
    .filter((note) => (filter.trashOnly ? note.deletedAt !== null : note.deletedAt === null))
    .filter((note) => !filter.notebookId || note.notebookId === filter.notebookId)
    .filter((note) =>
      filter.tagIds.every((id) => note.tags.includes(snapshot.tags.find((tag) => tag.id === id)?.name ?? '')),
    )
    .filter(
      (note) =>
        !query || `${note.title} ${note.plainText} ${note.tags.join(' ')}`.toLowerCase().includes(query),
    )
    .sort((a, b) => (filter.sort === 'titleAsc' ? a.title.localeCompare(b.title) : b.updatedAt - a.updatedAt))
    .map((note) => ({
      id: note.id,
      notebookId: note.notebookId,
      notebookName: snapshot.notebooks.find((item) => item.id === note.notebookId)?.name ?? 'Inbox',
      title: note.title || 'Untitled',
      snippet: note.plainText.slice(0, 260),
      createdAt: note.createdAt,
      updatedAt: note.updatedAt,
      deletedAt: note.deletedAt,
      tags: note.tags,
      hasAttachments: note.resources.length > 0,
      isShortcut: snapshot.shortcuts.some((item) => item.targetType === 'note' && item.targetId === note.id),
    }));
}

/** Browser-only demonstration adapter. The native application always uses SQLite. */
function demo<Args extends unknown[], Result>(operation: (...args: Args) => Result) {
  return (...args: Args): Promise<Result> => Promise.resolve().then(() => operation(...args));
}

export const browserApi: BevernoteApi = {
  bootstrap: demo(() => structuredClone(snapshot)),
  listNotes: demo(summaries),
  getNote: demo((id: string) => {
    const note = notes.get(id);
    if (!note) throw new Error('Note not found');
    return structuredClone(note);
  }),
  createNote: demo((notebookId: string | null) => {
    const timestamp = Date.now();
    const note: Note = {
      id: crypto.randomUUID(),
      notebookId: notebookId ?? inboxId,
      title: '',
      contentHtml: '<p></p>',
      originalEnml: null,
      plainText: '',
      createdAt: timestamp,
      updatedAt: timestamp,
      deletedAt: null,
      attributes: {},
      evernoteGuid: null,
      revision: 1,
      tags: [],
      resources: [],
    };
    notes.set(note.id, note);
    return structuredClone(note);
  }),
  saveNote: demo((input: SaveNote) => {
    const previous = notes.get(input.id);
    if (!previous) throw new Error('Note not found');
    for (const name of input.tags) {
      if (!snapshot.tags.some((tag) => tag.name === name)) {
        snapshot.tags.push({
          id: crypto.randomUUID(),
          name,
          parentId: null,
          color: null,
          createdAt: Date.now(),
          updatedAt: Date.now(),
        });
      }
    }
    const note = {
      ...previous,
      ...input,
      title: input.title.trim(),
      plainText: textOf(input.contentHtml),
      updatedAt: Date.now(),
      revision: previous.revision + 1,
    };
    notes.set(note.id, note);
    return structuredClone(note);
  }),
  setNoteTrashed: demo((id: string, trashed: boolean) => {
    const note = notes.get(id);
    if (note) notes.set(id, { ...note, deletedAt: trashed ? Date.now() : null });
  }),
  deleteNotePermanently: demo((id: string) => {
    notes.delete(id);
  }),
  createNotebook: demo((name: string, stackId: string | null) => {
    snapshot.notebooks.push({
      id: crypto.randomUUID(),
      name,
      stackId,
      createdAt: Date.now(),
      updatedAt: Date.now(),
      revision: 1,
    });
    return structuredClone(snapshot);
  }),
  setNotebookStack: demo((notebookId: string, stackId: string | null) => {
    const notebook = snapshot.notebooks.find((item) => item.id === notebookId);
    if (!notebook) throw new Error('Notebook not found');
    notebook.stackId = stackId;
    notebook.updatedAt = Date.now();
    return structuredClone(snapshot);
  }),
  createStack: demo((name: string) => {
    snapshot.stacks.push({
      id: crypto.randomUUID(),
      name,
      createdAt: Date.now(),
      updatedAt: Date.now(),
      revision: 1,
    });
    return structuredClone(snapshot);
  }),
  createTag: demo((name: string, parentId: string | null) => {
    snapshot.tags.push({
      id: crypto.randomUUID(),
      name,
      parentId,
      color: null,
      createdAt: Date.now(),
      updatedAt: Date.now(),
    });
    return structuredClone(snapshot);
  }),
  toggleShortcut: demo((targetType: string, targetId: string, label: string) => {
    const index = snapshot.shortcuts.findIndex(
      (item) => item.targetType === targetType && item.targetId === targetId,
    );
    if (index >= 0) snapshot.shortcuts.splice(index, 1);
    else
      snapshot.shortcuts.push({
        id: crypto.randomUUID(),
        targetType: targetType as 'note',
        targetId,
        label,
        sortOrder: 0,
        createdAt: Date.now(),
      });
    return structuredClone(snapshot);
  }),
  importEnex: demo((): ImportReport => {
    throw new Error('ENEX import requires the native Bevernote app.');
  }),
  organizeNotes: demo(
    (noteIds: string[], action: NoteOrganizationAction, notebookId: string | null, tags: string[]) => {
      const timestamp = Date.now();
      if (action === 'move' || action === 'copy') {
        if (!notebookId || !snapshot.notebooks.some((notebook) => notebook.id === notebookId))
          throw new Error('Choose a destination notebook.');
        if (action === 'move') {
          for (const id of noteIds) {
            const note = notes.get(id);
            if (!note) throw new Error('Note not found');
            notes.set(id, {
              ...note,
              notebookId,
              updatedAt: timestamp,
              revision: note.revision + 1,
            });
          }
        } else {
          for (const id of noteIds) {
            const original = notes.get(id);
            if (!original) throw new Error('Note not found');
            const copyId = crypto.randomUUID();
            notes.set(copyId, {
              ...structuredClone(original),
              id: copyId,
              notebookId,
              title: `${original.title.trim() || 'Untitled'} (copy)`,
              createdAt: timestamp,
              updatedAt: timestamp,
              evernoteGuid: null,
              revision: 1,
              resources: original.resources.map((resource) => ({
                ...structuredClone(resource),
                id: crypto.randomUUID(),
                noteId: copyId,
              })),
            });
          }
        }
        return noteIds.length;
      }
      const names = [...new Set(tags.map((tag) => tag.trim().replace(/^#/, '')).filter(Boolean))];
      let changed = 0;
      for (const id of noteIds) {
        const note = notes.get(id);
        if (!note) throw new Error('Note not found');
        const merged = [...note.tags];
        for (const name of names) {
          if (!merged.some((tag) => tag.toLowerCase() === name.toLowerCase())) merged.push(name);
          if (!snapshot.tags.some((tag) => tag.name.toLowerCase() === name.toLowerCase())) {
            snapshot.tags.push({
              id: crypto.randomUUID(),
              name,
              parentId: null,
              color: null,
              createdAt: timestamp,
              updatedAt: timestamp,
            });
          }
        }
        if (merged.length !== note.tags.length) {
          notes.set(id, { ...note, tags: merged, updatedAt: timestamp, revision: note.revision + 1 });
          changed += 1;
        }
      }
      return changed;
    },
  ),
  updateNoteTimestamps: demo((id: string, createdAt: number, updatedAt: number) => {
    const note = notes.get(id);
    if (!note) throw new Error('Note not found');
    if (createdAt < 0 || updatedAt < createdAt)
      throw new Error('Updated time must be at or after created time.');
    const updated = { ...note, createdAt, updatedAt, revision: note.revision + 1 };
    notes.set(id, updated);
    return structuredClone(updated);
  }),
  ocrEnabled: demo(() => false),
  setOcrEnabled: demo(() => {
    throw new Error('Local OCR requires the native Bevernote app.');
  }),
  exportEnex: demo(() => {
    throw new Error('ENEX export requires the native Bevernote app.');
  }),
  attachFile: demo((): Note => {
    throw new Error('Attachments require the native Bevernote app.');
  }),
  openAttachment: demo(() => {
    throw new Error('Opening attachments requires the native Bevernote app.');
  }),
  saveAttachment: demo(() => {
    throw new Error('Saving attachments requires the native Bevernote app.');
  }),
  syncNow: demo((): SyncReport => ({
    pushed: 0,
    pulled: 0,
    conflicts: 0,
    blobsUploaded: 0,
    blobsDownloaded: 0,
    syncedAt: Date.now(),
  })),
  pcloudStatus: demo(() => ({ connected: false, email: null, hostname: 'api.pcloud.com', lastSyncAt: null })),
  connectPcloud: demo(() => {
    throw new Error('pCloud connection requires the native Bevernote app.');
  }),
  connectPcloudToken: demo(() => {
    throw new Error('pCloud connection requires the native Bevernote app.');
  }),
  disconnectPcloud: demo(() => ({
    connected: false,
    email: null,
    hostname: 'api.pcloud.com',
    lastSyncAt: null,
  })),
};
