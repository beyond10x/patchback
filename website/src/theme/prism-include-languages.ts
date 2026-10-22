/**
 * Wraps the classic theme's grammar loader, then teaches the Bash grammar (and its `sh` and
 * `shell` aliases) the shape of a Patchback command line: the binary, its subcommands and the
 * files it reads and writes. Flags are already Bash `parameter` tokens.
 */
import type * as PrismNamespace from 'prismjs';
import includeLanguages from '@theme-original/prism-include-languages';

const BINARIES = ['patchback', 'cargo', 'task', 'npm', 'npx', 'modelspec', 'ess', 'aep', 'git'];
const FILE_EXTENSIONS = ['wav', 'patch', 'events', 'txt', 'meta', 'json', 'toml', 'yaml', 'yml', 'f32', 'u32', 'mspec', 'bin', 'rs', 'md'];

export default function prismIncludeLanguages(prism: typeof PrismNamespace): void {
  includeLanguages(prism);
  const bash = prism.languages.bash;
  if (!bash) return;
  const cli = {
    pattern: new RegExp(`(^|[;&|(]\\s*)(?:${BINARIES.join('|')})(?:[ \\t]+[a-z][a-z0-9-]*(?=[ \\t]|$)){0,2}`, 'm'),
    lookbehind: true,
    greedy: true,
    inside: {
      binary: /^\S+/,
      subcommand: /\S+/,
    },
  };
  const path = {
    pattern: new RegExp(`(^|[\\s=])(?:\\.{0,2}/)?[\\w.-]+(?:/[\\w.-]+)*\\.(?:${FILE_EXTENSIONS.join('|')})\\b`),
    lookbehind: true,
  };
  prism.languages.insertBefore('bash', 'function-name', {cli, path});
  prism.languages.sh = prism.languages.bash;
  prism.languages.shell = prism.languages.bash;
}
