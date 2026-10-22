import type {PrismTheme} from 'prism-react-renderer';

/**
 * Code colours in the landing page's signal palette (cyan, magenta, amber, lime, violet, orange).
 * Every token colour clears 4.5:1 against its background.
 */
type Palette = {
  background: string;
  text: string;
  comment: string;
  punctuation: string;
  keyword: string;
  string: string;
  literal: string;
  key: string;
  func: string;
  type: string;
  flag: string;
};

function theme(p: Palette): PrismTheme {
  return {
    plain: {color: p.text, backgroundColor: p.background},
    styles: [
      {types: ['comment', 'prolog', 'doctype', 'cdata', 'shebang'], style: {color: p.comment, fontStyle: 'italic'}},
      {types: ['punctuation', 'operator', 'file-descriptor'], style: {color: p.punctuation}},
      {types: ['keyword', 'selector', 'important', 'atrule', 'tag', 'deleted', 'subcommand'], style: {color: p.keyword}},
      {types: ['string', 'char', 'attr-value', 'regex', 'url', 'inserted', 'path'], style: {color: p.string}},
      {types: ['number', 'boolean', 'constant', 'symbol', 'lifetime-annotation', 'environment'], style: {color: p.literal}},
      {types: ['property', 'attr-name', 'key', 'macro', 'namespace', 'assign-left', 'table'], style: {color: p.key}},
      {types: ['function', 'function-definition', 'function-name', 'builtin'], style: {color: p.func}},
      {types: ['binary'], style: {color: p.func, fontWeight: '600'}},
      {types: ['class-name', 'type-definition'], style: {color: p.type}},
      {types: ['parameter', 'variable'], style: {color: p.flag}},
      {types: ['key', 'atrule'], languages: ['yaml'], style: {color: p.key}},
      {types: ['scalar', 'string'], languages: ['yaml'], style: {color: p.string}},
      {types: ['output'], style: {color: p.comment}},
      {types: ['important', 'bold'], style: {fontWeight: '600'}},
      {types: ['italic'], style: {fontStyle: 'italic'}},
    ],
  };
}

export const patchbackLight = theme({
  background: '#f6f4ff',
  text: '#1b1736',
  comment: '#66628a',
  punctuation: '#545077',
  keyword: '#a3168f',
  string: '#0b6b7d',
  literal: '#8a4f00',
  key: '#3b6e00',
  func: '#5b2fd6',
  type: '#7a3fc4',
  flag: '#b33a0b',
});

export const patchbackDark = theme({
  background: '#0d0b1f',
  text: '#e8e6ff',
  comment: '#8c89b5',
  punctuation: '#aaa7d2',
  keyword: '#ff6bd6',
  string: '#5fe6ea',
  literal: '#ffd23f',
  key: '#b6ff4a',
  func: '#a99bff',
  type: '#c99bff',
  flag: '#ffad66',
});
