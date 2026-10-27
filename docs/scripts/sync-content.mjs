import { agentAssets } from './agent-assets.mjs';
// Website pages are generated from the maintained Xecute guides.
import { readFileSync, writeFileSync, mkdirSync, readdirSync, existsSync, statSync } from 'node:fs';
import { resolve, dirname, relative, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
const site = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repo = resolve(site, '..');
const manifest = JSON.parse(readFileSync(resolve(site, 'content-manifest.json'), 'utf8'));
const output = resolve(site, 'content/docs');
const check = process.argv.includes('--check');
const github = 'https://github.com/morshedulmunna/xecute';
const url = (route) => route ? `/docs/${route}` : '/docs';
const slug = (title) => title.toLowerCase().replace(/`/g, '').replace(/[^\p{L}\p{N}_\- ]/gu, '').replace(/ /g, '-');
const read = (file) => readFileSync(resolve(repo, file), 'utf8');
function headings(markdown) {
  let fence = null;
  return markdown.split('\n').flatMap((line, index) => {
    const marker = /^\s*(`{3,}|~{3,})/.exec(line);
    if (marker) { if (!fence) fence = marker[1][0]; else if (fence === marker[1][0]) fence = null; return []; }
    const match = !fence && /^(#{1,6}) (.+)$/.exec(line);
    return match ? [{ level: match[1].length, title: match[2], line: index }] : [];
  });
}
const sources = new Map();
for (const page of manifest.pages) {
  if (!sources.has(page.source)) sources.set(page.source, []);
  sources.get(page.source).push(page);
}
const anchors = new Map();
for (const [file, pages] of sources) {
  const map = new Map(); let owner;
  const full = pages.find((page) => !page.heading);
  for (const heading of headings(read(file))) {
    const match = pages.find((page) => page.heading === heading.title);
    if (match) owner = match;
    else if (!owner || heading.level <= (owner.heading ? owner.level ?? 2 : 1)) owner = full;
    if (owner) map.set(slug(heading.title), { route: owner.route, top: heading.title === owner.heading || heading.level === 1 });
  }
  anchors.set(file, map);
}
function section(page) {
  const text = read(page.source);
  if (!page.heading) return text.replace(/^# .+\n+/, '');
  const all = headings(text);
  const start = all.find((heading) => heading.title === page.heading && heading.level === (page.level ?? 2));
  if (!start) throw new Error(`Missing heading: ${page.source} — ${page.heading}`);
  const end = all.find((heading) => heading.line > start.line && heading.level <= start.level);
  return text.split('\n').slice(start.line + 1, end?.line).join('\n').replace(/^(#{3,6}) /gm, (_, prefix) => `${'#'.repeat(Math.max(2, prefix.length - 1))} `);
}
function target(link, source) {
  if (/^(?:https?:|mailto:|tel:|\/docs(?:\/|$))/.test(link)) return link;
  if (link.startsWith('#')) { const mapped = anchors.get(source)?.get(link.slice(1)); return mapped ? `${url(mapped.route)}${mapped.top ? '' : link}` : link; }
  const [file, anchor] = link.split('#');
  const absolute = resolve(dirname(resolve(repo, source)), file);
  const repoFile = relative(repo, absolute).replaceAll('\\', '/');
  const mapped = anchors.get(repoFile)?.get(anchor);
  const page = mapped ? manifest.pages.find((entry) => entry.route === mapped.route) : sources.get(repoFile)?.find((entry) => !entry.heading);
  if (page) return `${url(page.route)}${anchor && !mapped?.top ? `#${anchor}` : ''}`;
  const type = existsSync(absolute) && statSync(absolute).isDirectory() ? 'tree' : 'blob';
  return `${github}/${type}/main/${encodeURI(repoFile)}${anchor ? `#${anchor}` : ''}`;
}
function links(text, source) {
  let fence = false; let rust = false;
  return text.split('\n').map((line) => {
    if (/^\s*```/.test(line)) { fence = !fence; rust = fence && /```rust/.test(line); return line; }
    if (fence) return rust ? line.replace(/^# (.*)$/, '$1') : line;
    return line.replace(/(?<!!)\[([^\]]+)\]\(([^\s)]+)\)/g, (_, label, link) => `[${label}](${target(link, source)})`);
  }).join('\n');
}
const expected = new Map();
const agentPages = new Map();
for (const page of manifest.pages) {
  const introduction = page.intro ? `${page.intro}\n\n` : '';
  const next = page.next ? `\n\n## Keep learning\n\n[${page.next.title}](${url(page.next.route)}). Return to the [learning path](/docs) whenever you need a different topic.` : '';
  const body = introduction + links(section(page), page.source).trim() + next;
  agentPages.set(page.route, body);
  const anchor = page.heading ? `#${slug(page.heading)}` : '';
  expected.set(resolve(output, `${page.route || 'index'}.md`), `---\ntitle: ${JSON.stringify(page.title)}\ndescription: ${JSON.stringify(page.description)}\n---\n\n${body}\n\n---\n\n[Maintained source](${github}/blob/main/${page.source}${anchor}).\n`);
}
expected.set(resolve(output, 'meta.json'), JSON.stringify({ title: 'Xecute documentation', pages: ['index', ...manifest.categories.map((category) => category.slug)] }, null, 2) + '\n');
for (const category of manifest.categories) expected.set(resolve(output, category.slug, 'meta.json'), JSON.stringify({ title: category.title, pages: category.pages }, null, 2) + '\n');
for (const [file, content] of agentAssets(manifest, agentPages, read('docs/agent-skill/xecute/SKILL.md'))) expected.set(resolve(site, 'public', file), content);
let stale = 0;
for (const [file, content] of expected) {
  if (check) { if (!existsSync(file) || !readFileSync(file).equals(Buffer.from(content))) { console.error(`Stale documentation: ${relative(repo, file)}`); stale++; } }
  else { mkdirSync(dirname(file), { recursive: true }); writeFileSync(file, content); }
}
function files(dir) { return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => entry.isDirectory() ? files(resolve(dir, entry.name)) : [resolve(dir, entry.name)]); }
for (const file of files(output)) if (['.md', '.mdx', '.json'].includes(extname(file)) && !expected.has(file)) { console.error(`Unexpected content: ${relative(repo, file)}`); stale++; }
if (stale) process.exitCode = 1;
else console.log(`${check ? 'Verified' : 'Generated'} ${manifest.pages.length} Xecute documentation pages.`);
