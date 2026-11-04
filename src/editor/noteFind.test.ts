import { getSchema } from '@tiptap/core';
import { describe, expect, it } from 'vitest';

import { createExtensions } from './extensions';
import { findTextMatches } from './noteFind';

const schema = getSchema(createExtensions());

function documentWithText(text: string) {
  return schema.nodeFromJSON({
    type: 'doc',
    content: [{ type: 'paragraph', content: [{ type: 'text', text }] }],
  });
}

describe('note content find', () => {
  it('maps all case-insensitive matches to ProseMirror positions', () => {
    const doc = documentWithText('Alpha beta ALPHA');
    expect(findTextMatches(doc, 'alpha')).toEqual([
      { from: 1, to: 6 },
      { from: 12, to: 17 },
    ]);
  });

  it('does not include titles or library interface text', () => {
    const doc = documentWithText('Only the note body');
    expect(findTextMatches(doc, 'Search notes')).toEqual([]);
    expect(findTextMatches(doc, 'note body')).toHaveLength(1);
  });

  it('supports multi-paragraph search and caps very large results', () => {
    const doc = schema.nodeFromJSON({
      type: 'doc',
      content: [
        { type: 'paragraph', content: [{ type: 'text', text: 'one two' }] },
        { type: 'paragraph', content: [{ type: 'text', text: 'two three' }] },
      ],
    });
    expect(findTextMatches(doc, 'two')).toHaveLength(2);
    expect(findTextMatches(documentWithText('aaaa'), 'a', 2)).toHaveLength(2);
  });
});
