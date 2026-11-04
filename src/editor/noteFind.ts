import { Extension } from '@tiptap/core';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';
import type { Editor } from '@tiptap/react';

const MAX_MATCHES = 2000;
const key = new PluginKey<FindState>('bevernoteFind');

export interface FindMatch {
  from: number;
  to: number;
}

export interface FindState {
  query: string;
  active: number;
  matches: FindMatch[];
  decorations: DecorationSet;
}

interface FindUpdate {
  query: string;
  active?: number;
}

/** Search text nodes only: titles, sidebars, and search results are never included. */
export function findTextMatches(doc: ProseMirrorNode, query: string, limit = MAX_MATCHES): FindMatch[] {
  const needle = query.trim().toLocaleLowerCase();
  if (!needle) return [];
  const matches: FindMatch[] = [];
  doc.descendants((node, position) => {
    if (matches.length >= limit) return false;
    if (!node.isText || !node.text) return true;
    const haystack = node.text.toLocaleLowerCase();
    let offset = 0;
    while (matches.length < limit) {
      const index = haystack.indexOf(needle, offset);
      if (index < 0) break;
      matches.push({ from: position + index, to: position + index + needle.length });
      offset = index + Math.max(needle.length, 1);
    }
    return true;
  });
  return matches;
}

function createState(doc: ProseMirrorNode, update: FindUpdate): FindState {
  const matches = findTextMatches(doc, update.query);
  const active = matches.length
    ? (((update.active ?? 0) % matches.length) + matches.length) % matches.length
    : 0;
  const decorations = matches.map((match, index) =>
    Decoration.inline(match.from, match.to, {
      class: `note-find-match${index === active ? ' active' : ''}`,
      'data-find-match': String(index),
    }),
  );
  return { query: update.query, active, matches, decorations: DecorationSet.create(doc, decorations) };
}

export const NoteFindExtension = Extension.create({
  name: 'bevernoteFind',
  addProseMirrorPlugins() {
    return [
      new Plugin<FindState>({
        key,
        state: {
          init: (_, { doc }) => createState(doc, { query: '' }),
          apply(transaction, previous) {
            const update = transaction.getMeta(key) as FindUpdate | undefined;
            if (update) return createState(transaction.doc, update);
            return {
              ...previous,
              matches: findTextMatches(transaction.doc, previous.query),
              decorations: previous.decorations.map(transaction.mapping, transaction.doc),
            };
          },
        },
        props: {
          decorations(state) {
            return key.getState(state)?.decorations ?? DecorationSet.empty;
          },
        },
      }),
    ];
  },
});

export function updateNoteFind(editor: Editor, query: string, active = 0) {
  editor.view.dispatch(editor.state.tr.setMeta(key, { query, active }));
  const state = key.getState(editor.state);
  if (state?.query && state.matches.length) {
    const element = editor.view.dom.querySelector(`[data-find-match="${state.active}"]`);
    element?.scrollIntoView({ block: 'center', behavior: 'smooth' });
  }
  return state;
}

export function noteFindState(editor: Editor): FindState | undefined {
  return key.getState(editor.state);
}
