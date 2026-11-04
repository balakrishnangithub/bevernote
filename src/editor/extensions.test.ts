import { Editor } from '@tiptap/core';
import { describe, expect, it } from 'vitest';

import { createExtensions } from './extensions';

function editorWith(content: string) {
  return new Editor({
    element: document.createElement('div'),
    extensions: createExtensions(),
    content,
  });
}

describe('editor serialization', () => {
  it('preserves Evernote media, todo, and encrypted blocks', () => {
    const source =
      '<p>Before <en-todo checked="true"></en-todo><en-media hash="abcd1234" type="image/png" width="640"></en-media><en-crypt hint="Hint" cipher="RC2" length="64">payload==</en-crypt></p>';
    const editor = editorWith(source);
    const html = editor.getHTML();
    expect(html).toContain('<en-todo checked="true"');
    expect(html).toContain('<en-media');
    expect(html).toContain('hash="abcd1234"');
    expect(html).toContain('type="image/png"');
    expect(html).toContain('payload==');
    expect(html).toContain('cipher="RC2"');
    editor.destroy();
  });

  it('retains task-list structure and check state after edits', () => {
    const editor = editorWith(
      '<ul data-type="taskList"><li data-type="taskItem" data-checked="false"><p>Offline</p></li></ul>',
    );
    editor.commands.updateAttributes('taskItem', { checked: true });
    expect(editor.getHTML()).toContain('data-checked="true"');
    expect(editor.getHTML()).toContain('data-type="taskList"');
    editor.destroy();
  });

  it('serializes typography, alignment, headings, and indentation', () => {
    const editor = editorWith('<p>Formatted</p>');
    editor.commands.selectAll();
    editor
      .chain()
      .focus()
      .toggleBold()
      .setFontFamily('Arial')
      .setColor('#112233')
      .setMark('textStyle', { fontSize: '18px' })
      .run();
    editor.commands.setTextAlign('center');
    editor.commands.updateAttributes('paragraph', { indent: 2 });
    const html = editor.getHTML();
    expect(html).toContain('<strong>');
    expect(html).toContain('font-family: Arial');
    expect(html).toContain('font-size: 18px');
    expect(html).toContain('color: rgb(17, 34, 51)');
    expect(html).toContain('text-align: center');
    expect(html).toContain('margin-left: 4em');
    editor.commands.setHeading({ level: 3 });
    expect(editor.getHTML()).toContain('<h3');
    editor.destroy();
  });
});
