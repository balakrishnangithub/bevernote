import { create } from 'zustand';

import { api } from '../lib/api';
import {
  emptyFilter,
  type LibrarySnapshot,
  type Note,
  type NoteFilter,
  type NoteOrganizationAction,
  type NoteSummary,
  type SaveNote,
} from '../types';

interface LibraryState {
  snapshot: LibrarySnapshot;
  notes: NoteSummary[];
  selectedNote: Note | null;
  filter: NoteFilter;
  loading: boolean;
  busy: boolean;
  error: string | null;
  initialize: () => Promise<void>;
  refreshNotes: () => Promise<void>;
  refreshSnapshot: () => Promise<void>;
  selectNote: (id: string | null) => Promise<void>;
  setFilter: (filter: Partial<NoteFilter>) => Promise<void>;
  createNote: () => Promise<void>;
  saveNote: (input: SaveNote) => Promise<void>;
  createNotebook: (name: string, stackId?: string) => Promise<void>;
  setNotebookStack: (notebookId: string, stackId: string | null) => Promise<void>;
  createStack: (name: string) => Promise<void>;
  createTag: (name: string) => Promise<void>;
  toggleShortcut: (targetType: string, targetId: string, label: string) => Promise<void>;
  trashSelected: () => Promise<void>;
  restoreSelected: () => Promise<void>;
  deleteSelected: () => Promise<void>;
  attachFile: (path: string) => Promise<Note | null>;
  organizeNotes: (
    noteIds: string[],
    action: NoteOrganizationAction,
    notebookId: string | null,
    tags: string[],
  ) => Promise<number>;
  updateNoteTimestamps: (id: string, createdAt: number, updatedAt: number) => Promise<void>;
  clearError: () => void;
}

export const useLibraryStore = create<LibraryState>((set, get) => ({
  snapshot: { stacks: [], notebooks: [], tags: [], shortcuts: [] },
  notes: [],
  selectedNote: null,
  filter: { ...emptyFilter },
  loading: true,
  busy: false,
  error: null,
  initialize: async () => {
    try {
      const snapshot = await api.bootstrap();
      set({ snapshot, loading: false });
      await get().refreshNotes();
    } catch (error) {
      set({ loading: false, error: message(error) });
    }
  },
  refreshNotes: async () => {
    try {
      const notes = await api.listNotes(get().filter);
      set({ notes, error: null });
    } catch (error) {
      set({ error: message(error) });
    }
  },
  refreshSnapshot: async () => {
    try {
      set({ snapshot: await api.bootstrap() });
    } catch (error) {
      set({ error: message(error) });
    }
  },
  selectNote: async (id) => {
    if (!id) {
      set({ selectedNote: null });
      return;
    }
    try {
      set({ selectedNote: await api.getNote(id), error: null });
    } catch (error) {
      set({ error: message(error) });
    }
  },
  setFilter: async (partial) => {
    set((state) => ({ filter: { ...state.filter, ...partial, offset: 0 } }));
    await get().refreshNotes();
  },
  createNote: async () => {
    try {
      const note = await api.createNote(get().filter.notebookId);
      await get().refreshNotes();
      set({ selectedNote: note });
    } catch (error) {
      set({ error: message(error) });
    }
  },
  saveNote: async (input) => {
    try {
      const selectedNote = await api.saveNote(input);
      set({ selectedNote, error: null });
      await Promise.all([get().refreshNotes(), get().refreshSnapshot()]);
    } catch (error) {
      set({ error: message(error) });
      throw error;
    }
  },
  createNotebook: async (name, stackId) => {
    try {
      set({ snapshot: await api.createNotebook(name, stackId ?? null) });
    } catch (error) {
      set({ error: message(error) });
    }
  },
  setNotebookStack: async (notebookId, stackId) => {
    try {
      set({ snapshot: await api.setNotebookStack(notebookId, stackId) });
    } catch (error) {
      set({ error: message(error) });
    }
  },
  createStack: async (name) => {
    try {
      set({ snapshot: await api.createStack(name) });
    } catch (error) {
      set({ error: message(error) });
    }
  },
  createTag: async (name) => {
    try {
      set({ snapshot: await api.createTag(name, null) });
    } catch (error) {
      set({ error: message(error) });
    }
  },
  toggleShortcut: async (targetType, targetId, label) => {
    try {
      set({ snapshot: await api.toggleShortcut(targetType, targetId, label) });
      await get().refreshNotes();
    } catch (error) {
      set({ error: message(error) });
    }
  },
  trashSelected: async () => {
    const note = get().selectedNote;
    if (!note) return;
    await api.setNoteTrashed(note.id, true);
    set({ selectedNote: null });
    await get().refreshNotes();
  },
  restoreSelected: async () => {
    const note = get().selectedNote;
    if (!note) return;
    await api.setNoteTrashed(note.id, false);
    await get().refreshNotes();
  },
  deleteSelected: async () => {
    const note = get().selectedNote;
    if (!note) return;
    await api.deleteNotePermanently(note.id);
    set({ selectedNote: null });
    await get().refreshNotes();
  },
  attachFile: async (path) => {
    const note = get().selectedNote;
    if (!note) return null;
    try {
      const updated = await api.attachFile(note.id, path);
      set({ selectedNote: updated });
      return updated;
    } catch (error) {
      set({ error: message(error) });
      return null;
    }
  },
  organizeNotes: async (noteIds, action, notebookId, tags) => {
    set({ busy: true, error: null });
    try {
      const changed = await api.organizeNotes(noteIds, action, notebookId, tags);
      await get().refreshSnapshot();
      await get().refreshNotes();
      const selectedId = get().selectedNote?.id;
      if (selectedId && action !== 'copy') await get().selectNote(selectedId);
      set({ busy: false, error: null });
      return changed;
    } catch (error) {
      set({ busy: false, error: message(error) });
      throw error;
    }
  },
  updateNoteTimestamps: async (id, createdAt, updatedAt) => {
    set({ busy: true, error: null });
    try {
      const selectedNote = await api.updateNoteTimestamps(id, createdAt, updatedAt);
      set({ selectedNote, busy: false, error: null });
      await Promise.all([get().refreshNotes(), get().refreshSnapshot()]);
    } catch (error) {
      set({ busy: false, error: message(error) });
      throw error;
    }
  },
  clearError: () => set({ error: null }),
}));

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
