import { invoke } from '@tauri-apps/api/core';

import type {
  ImportReport,
  LibrarySnapshot,
  Note,
  NoteFilter,
  NoteOrganizationAction,
  NoteSummary,
  PCloudStatus,
  SaveNote,
  SyncStatus,
} from '../types';
import { browserApi } from './browserApi';

export interface BevernoteApi {
  bootstrap: () => Promise<LibrarySnapshot>;
  listNotes: (filter: NoteFilter) => Promise<NoteSummary[]>;
  getNote: (id: string) => Promise<Note>;
  createNote: (notebookId: string | null) => Promise<Note>;
  saveNote: (input: SaveNote) => Promise<Note>;
  setNoteTrashed: (id: string, trashed: boolean) => Promise<void>;
  deleteNotePermanently: (id: string) => Promise<void>;
  createNotebook: (name: string, stackId: string | null) => Promise<LibrarySnapshot>;
  createStack: (name: string) => Promise<LibrarySnapshot>;
  createTag: (name: string, parentId: string | null) => Promise<LibrarySnapshot>;
  toggleShortcut: (targetType: string, targetId: string, label: string) => Promise<LibrarySnapshot>;
  importEnex: (path: string, notebookName: string | null, importId: string) => Promise<ImportReport>;
  organizeNotes: (
    noteIds: string[],
    action: NoteOrganizationAction,
    notebookId: string | null,
    tags: string[],
  ) => Promise<number>;
  updateNoteTimestamps: (id: string, createdAt: number, updatedAt: number) => Promise<Note>;
  ocrEnabled: () => Promise<boolean>;
  setOcrEnabled: (enabled: boolean) => Promise<void>;
  exportEnex: (noteIds: string[], path: string) => Promise<void>;
  attachFile: (noteId: string, path: string) => Promise<Note>;
  openAttachment: (resourceId: string) => Promise<void>;
  saveAttachment: (resourceId: string, destination: string) => Promise<void>;
  setNotebookStack: (notebookId: string, stackId: string | null) => Promise<LibrarySnapshot>;
  syncNow: () => Promise<SyncStatus>;
  syncStatus: () => Promise<SyncStatus>;
  pcloudStatus: () => Promise<PCloudStatus>;
  connectPcloud: () => Promise<PCloudStatus>;
  connectPcloudToken: (accessToken: string, hostname: string) => Promise<PCloudStatus>;
  disconnectPcloud: () => Promise<PCloudStatus>;
}

const tauriApi: BevernoteApi = {
  bootstrap: () => invoke('bootstrap'),
  listNotes: (filter) => invoke('list_notes', { filter }),
  getNote: (id) => invoke('get_note', { id }),
  createNote: (notebookId) => invoke('create_note', { notebookId }),
  saveNote: (input) => invoke('save_note', { input }),
  setNoteTrashed: (id, trashed) => invoke('set_note_trashed', { id, trashed }),
  deleteNotePermanently: (id) => invoke('delete_note_permanently', { id }),
  createNotebook: (name, stackId) => invoke('create_notebook', { name, stackId }),
  createStack: (name) => invoke('create_stack', { name }),
  createTag: (name, parentId) => invoke('create_tag', { name, parentId }),
  toggleShortcut: (targetType, targetId, label) => invoke('toggle_shortcut', { targetType, targetId, label }),
  importEnex: (path, notebookName, importId) => invoke('import_enex_file', { path, notebookName, importId }),
  organizeNotes: (noteIds, action, notebookId, tags) =>
    invoke('organize_notes', { noteIds, action, notebookId, tags }),
  updateNoteTimestamps: (id, createdAt, updatedAt) =>
    invoke('update_note_timestamps', { id, createdAt, updatedAt }),
  ocrEnabled: () => invoke('ocr_enabled'),
  setOcrEnabled: (enabled) => invoke('set_ocr_enabled', { enabled }),
  exportEnex: (noteIds, path) => invoke('export_enex_file', { noteIds, path }),
  attachFile: (noteId, path) => invoke('attach_file', { noteId, path }),
  openAttachment: (resourceId) => invoke('open_attachment', { resourceId }),
  saveAttachment: (resourceId, destination) => invoke('save_attachment', { resourceId, destination }),
  setNotebookStack: (notebookId, stackId) => invoke('set_notebook_stack', { notebookId, stackId }),
  syncNow: () => invoke('sync_now'),
  syncStatus: () => invoke('sync_status'),
  pcloudStatus: () => invoke('pcloud_status'),
  connectPcloud: () => invoke('connect_pcloud'),
  connectPcloudToken: (accessToken, hostname) => invoke('connect_pcloud_token', { accessToken, hostname }),
  disconnectPcloud: () => invoke('disconnect_pcloud'),
};

export const isNative = '__TAURI_INTERNALS__' in window;
export const api: BevernoteApi = isNative ? tauriApi : browserApi;

export function resourceUrl(reference: string): string {
  if (!isNative) return reference === 'demo-pdf' ? '/demo-attachments/demo-pdf' : '';
  return window.location.protocol === 'http:'
    ? `http://bever.localhost/${encodeURIComponent(reference)}`
    : `bever://localhost/${encodeURIComponent(reference)}`;
}
