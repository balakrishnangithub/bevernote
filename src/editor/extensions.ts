import { Extension, Node, mergeAttributes } from '@tiptap/core';
import Color from '@tiptap/extension-color';
import FontFamily from '@tiptap/extension-font-family';
import Highlight from '@tiptap/extension-highlight';
import Image from '@tiptap/extension-image';
import Link from '@tiptap/extension-link';
import Paragraph from '@tiptap/extension-paragraph';
import Subscript from '@tiptap/extension-subscript';
import Superscript from '@tiptap/extension-superscript';
import { Table } from '@tiptap/extension-table';
import TableCell from '@tiptap/extension-table-cell';
import TableHeader from '@tiptap/extension-table-header';
import TableRow from '@tiptap/extension-table-row';
import TaskItem from '@tiptap/extension-task-item';
import TaskList from '@tiptap/extension-task-list';
import TextAlign from '@tiptap/extension-text-align';
import { TextStyle } from '@tiptap/extension-text-style';
import Underline from '@tiptap/extension-underline';
import StarterKit from '@tiptap/starter-kit';
import { ReactNodeViewRenderer } from '@tiptap/react';

import { AttachmentView, EncryptedBlockView, TodoView } from './nodeViews';
import { NoteFindExtension } from './noteFind';

export const FontSize = Extension.create({
  name: 'fontSize',
  addGlobalAttributes() {
    return [
      {
        types: ['textStyle'],
        attributes: {
          fontSize: {
            default: null,
            parseHTML: (element) => element.style.fontSize || null,
            renderHTML: (attributes) =>
              attributes.fontSize ? { style: `font-size: ${attributes.fontSize}` } : {},
          },
        },
      },
    ];
  },
});

export const Indent = Extension.create({
  name: 'indent',
  addGlobalAttributes() {
    return [
      {
        types: ['paragraph', 'heading'],
        attributes: {
          indent: {
            default: 0,
            parseHTML: (element) => {
              const margin = Number.parseInt(
                element.style.marginLeft || element.style.paddingLeft || '0',
                10,
              );
              return Number.isFinite(margin) ? Math.round(margin / 32) : 0;
            },
            renderHTML: (attributes) =>
              Number(attributes.indent) > 0
                ? { style: `margin-left: ${Number(attributes.indent) * 2}em` }
                : {},
          },
        },
      },
    ];
  },
});

const EvernoteParagraph = Paragraph.extend({
  parseHTML() {
    return [{ tag: 'p' }, { tag: 'div:not([data-type])' }];
  },
});

export const EnMedia = Node.create({
  name: 'enMedia',
  inline: true,
  group: 'inline',
  atom: true,
  draggable: true,
  addAttributes() {
    return {
      hash: { default: '', parseHTML: (element) => element.getAttribute('hash') },
      type: { default: 'application/octet-stream', parseHTML: (element) => element.getAttribute('type') },
      width: { default: null, parseHTML: (element) => element.getAttribute('width') },
      height: { default: null, parseHTML: (element) => element.getAttribute('height') },
      style: { default: null, parseHTML: (element) => element.getAttribute('style') },
      title: { default: null, parseHTML: (element) => element.getAttribute('title') },
      alt: { default: null, parseHTML: (element) => element.getAttribute('alt') },
    };
  },
  parseHTML() {
    return [{ tag: 'en-media' }];
  },
  renderHTML({ HTMLAttributes }) {
    return ['en-media', mergeAttributes(HTMLAttributes)];
  },
  addNodeView() {
    return ReactNodeViewRenderer(AttachmentView);
  },
});

export const EnTodo = Node.create({
  name: 'enTodo',
  inline: true,
  group: 'inline',
  atom: true,
  addAttributes() {
    return {
      checked: {
        default: 'false',
        parseHTML: (element) => element.getAttribute('checked') ?? 'false',
      },
    };
  },
  parseHTML() {
    return [{ tag: 'en-todo' }];
  },
  renderHTML({ HTMLAttributes }) {
    return ['en-todo', mergeAttributes(HTMLAttributes)];
  },
  addNodeView() {
    return ReactNodeViewRenderer(TodoView);
  },
});

export const EnCrypt = Node.create({
  name: 'enCrypt',
  inline: true,
  group: 'inline',
  atom: true,
  addAttributes() {
    return {
      hint: { default: null, parseHTML: (element) => element.getAttribute('hint') },
      cipher: { default: null, parseHTML: (element) => element.getAttribute('cipher') },
      length: { default: null, parseHTML: (element) => element.getAttribute('length') },
      payload: {
        default: '',
        parseHTML: (element) => element.textContent ?? '',
        renderHTML: () => ({}),
      },
    };
  },
  parseHTML() {
    return [{ tag: 'en-crypt' }];
  },
  renderHTML({ node, HTMLAttributes }) {
    return ['en-crypt', mergeAttributes(HTMLAttributes), String(node.attrs.payload ?? '')];
  },
  addNodeView() {
    return ReactNodeViewRenderer(EncryptedBlockView);
  },
});

export function createExtensions() {
  return [
    StarterKit.configure({
      paragraph: false,
      link: false,
      underline: false,
      heading: { levels: [1, 2, 3, 4] },
      codeBlock: { exitOnTripleEnter: true },
    }),
    EvernoteParagraph,
    TextStyle,
    FontSize,
    FontFamily,
    Color,
    Highlight.configure({ multicolor: true }),
    Underline,
    Subscript,
    Superscript,
    TextAlign.configure({ types: ['paragraph', 'heading'] }),
    Indent,
    TaskList,
    TaskItem.configure({ nested: true }),
    Link.configure({
      openOnClick: false,
      autolink: true,
      linkOnPaste: true,
      protocols: ['bevernote', 'evernote'],
      HTMLAttributes: { rel: 'noopener noreferrer' },
    }),
    Table.configure({ resizable: true, lastColumnResizable: true }),
    TableRow,
    TableHeader,
    TableCell,
    Image.configure({ inline: true, allowBase64: false }),
    EnMedia,
    EnTodo,
    EnCrypt,
    NoteFindExtension,
  ];
}
