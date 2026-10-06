// Candidate 10: the scoped parser-major override retains Tailwind 3 behavior.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { test } from 'node:test';
import postcss from 'postcss';
import tailwindcss from 'tailwindcss';
import nesting from 'tailwindcss/nesting/index.js';

const require = createRequire(import.meta.url);
const fromTailwind = createRequire(require.resolve('tailwindcss'));
const fromNested = createRequire(fromTailwind.resolve('postcss-nested'));

test('patched scoped parsers preserve complex selectors and reject malformed input', () => {
  for (const from of [fromTailwind, fromNested]) {
    assert.equal(from('postcss-selector-parser/package.json').version, '7.1.6');
    const parser = from('postcss-selector-parser');
    const selector = String.raw`.card:is(.primary,.secondary):not([data-note="comma, and )"]) > .child\:token:hover`;
    assert.equal(parser().astSync(selector).toString(), selector);
    const nested = `${':is('.repeat(32)}.leaf${')'.repeat(32)}`;
    assert.equal(parser().astSync(nested).toString(), nested);
    assert.throws(() => parser().astSync(nested.slice(0, -1)));
    assert.throws(() => parser().astSync('a[title="unterminated]'));
  }
});

test('Tailwind utilities, variants, apply and nested selectors survive the override', async () => {
  const css = String.raw`
    @tailwind utilities;
    .card:is(.primary,.secondary):not([data-note="comma, and )"]) {
      @apply px-4 font-bold;
      & > .child\:token:hover { @apply text-red-500; }
    }
  `;
  const result = await postcss([
    nesting(),
    tailwindcss({
      content: [{ raw: '<div class="flex hover:bg-blue-500 md:grid group-hover:font-bold w-[13px]"></div>' }],
      corePlugins: { preflight: false },
    }),
  ]).process(css, { from: undefined, map: false });
  assert.match(result.css, /display:\s*flex/);
  assert.match(result.css, /display:\s*grid/);
  assert.match(result.css, /@media\s*\(min-width:\s*768px\)/);
  assert.ok(result.css.includes(String.raw`.hover\:bg-blue-500:hover`));
  assert.ok(result.css.includes(String.raw`.group:hover .group-hover\:font-bold`));
  assert.match(result.css, /width:\s*13px/);
  assert.match(result.css, /padding-left:\s*1rem/);
  assert.match(result.css, /font-weight:\s*700/);
  assert.ok(result.css.includes(String.raw`> .child\:token:hover`));
  assert.doesNotMatch(result.css, /@apply|@tailwind|&\s*>/);
});
