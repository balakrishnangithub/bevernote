export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };
export type Attributes = Record<string, JsonValue>;

export interface Stack {
  id: string;
  name: string;
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export interface Notebook {
  id: string;
  name: string;
  stackId: string | null;
  createdAt: number;
  updatedAt: number;
  revision: number;
}

export interface Tag {
  id: string;
  name: string;
  parentId: string | null;
  color: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface Shortcut {
  id: string;
  targetType: 'note' | 'notebook' | 'stack' | 'tag' | 'search';
  targetId: string;
  label: string;
  sortOrder: number;
  createdAt: number;
}

export interface Resource {
  id: string;
  noteId: string;
  sha256: string;
  md5: string;
  mime: string;
  fileName: string | null;
  size: number;
  width: number | null;
  height: number | null;
  duration: number | null;
  recognitionXml: string | null;
  recognitionText: string | null;
  attributes: Attributes;
  alternateSha256: string | null;
  position: number;
}

export interface Note {
  id: string;
  notebookId: string;
  title: string;
  contentHtml: string;
  originalEnml: string | null;
  plainText: string;
  createdAt: number;
  updatedAt: number;
  deletedAt: number | null;
  attributes: Attributes;
  evernoteGuid: string | null;
  revision: number;
  tags: string[];
  resources: Resource[];
}

export interface NoteSummary {
  id: string;
  notebookId: string;
  notebookName: string;
  title: string;
  snippet: string;
  createdAt: number;
  updatedAt: number;
  deletedAt: number | null;
  tags: string[];
  hasAttachments: boolean;
  isShortcut: boolean;
}

export interface SaveNote {
  id: string;
  notebookId: string;
  title: string;
  contentHtml: string;
  attributes: Attributes;
  tags: string[];
}

export type NoteSort =
  'relevance' | 'updatedDesc' | 'updatedAsc' | 'createdDesc' | 'createdAsc' | 'titleAsc' | 'titleDesc';

export interface NoteFilter {
  query: string;
  notebookId: string | null;
  stackId: string | null;
  tagIds: string[];
  includeTrash: boolean;
  trashOnly: boolean;
  createdFrom: number | null;
  createdTo: number | null;
  updatedFrom: number | null;
  updatedTo: number | null;
  contains: string | null;
  sort: NoteSort;
  limit: number;
  offset: number;
}

export const emptyFilter: NoteFilter = {
  query: '',
  notebookId: null,
  stackId: null,
  tagIds: [],
  includeTrash: false,
  trashOnly: false,
  createdFrom: null,
  createdTo: null,
  updatedFrom: null,
  updatedTo: null,
  contains: null,
  sort: 'updatedDesc',
  limit: 200,
  offset: 0,
};

export interface LibrarySnapshot {
  stacks: Stack[];
  notebooks: Notebook[];
  tags: Tag[];
  shortcuts: Shortcut[];
}

export interface ImportReport {
  notesImported: number;
  notesSkipped: number;
  resourcesImported: number;
  bytesImported: number;
  notebookId: string;
  warnings: string[];
}

export interface ImportProgress {
  importId: string;
  phase:
    'preparing' | 'notebookReady' | 'reading' | 'processing' | 'noteImported' | 'finalizing' | 'complete';
  notebookId: string;
  noteIndex: number;
  noteTitle: string | null;
  notesImported: number;
  notesSkipped: number;
  resourcesImported: number;
  bytesRead: number;
  totalBytes: number;
}

export type NoteOrganizationAction = 'move' | 'copy' | 'addTags';

export interface PCloudStatus {
  connected: boolean;
  email: string | null;
  hostname: string;
  lastSyncAt: number | null;
}

export interface SyncReport {
  pushed: number;
  pulled: number;
  conflicts: number;
  blobsUploaded: number;
  blobsDownloaded: number;
  syncedAt: number;
}
