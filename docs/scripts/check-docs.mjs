import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const repo = resolve(root, '..');
const manifest = JSON.parse(readFileSync(resolve(root, 'content-manifest.json'), 'utf8'));
const routes = new Set(manifest.pages.map((page) => page.route ? `/docs/${page.route}` : '/docs'));
const errors = [];
for (const page of manifest.pages) {
  const file = resolve(root, 'content/docs', `${page.route || 'index'}.md`);
  if (!existsSync(file)) { errors.push(`Missing page: ${file}`); continue; }
  const text = readFileSync(file, 'utf8');
  if (!/^---\ntitle: .+\ndescription: .+\n---/.test(text)) errors.push(`Invalid frontmatter: ${page.route}`);
  if (/\b(?:ironic::|cargo install ironic|Ironic Framework|nestrs::)\b/i.test(text)) errors.push(`Foreign framework content: ${page.route}`);
  let fence = false;
  for (const line of text.split('\n')) {
    if (/^\s*```/.test(line)) { fence = !fence; continue; }
    if (fence) continue;
    for (const match of line.matchAll(/\]\((\/docs[^\s)]*)\)/g)) if (!routes.has(match[1].split('#')[0].replace(/\/$/, ''))) errors.push(`Broken link in ${page.route}: ${match[1]}`);
    for (const match of line.matchAll(/\]\((?!https?:|mailto:|#|\/)([^\s)]*\.md(?:#[^\s)]*)?)\)/g)) errors.push(`Unresolved Markdown link: ${page.route} → ${match[1]}`);
  }
  if (fence) errors.push(`Unclosed fence: ${page.route}`);
}
for (const category of manifest.categories) for (const page of category.pages) if (!routes.has(`/docs/${category.slug}/${page}`)) errors.push(`Missing sidebar page: ${category.slug}/${page}`);
const navigable = new Set(['/docs', ...manifest.categories.flatMap((category) => category.pages.map((page) => `/docs/${category.slug}/${page}`))]);
for (const route of routes) if (!navigable.has(route)) errors.push(`Orphaned page outside sidebar: ${route}`);
const cargo = readFileSync(resolve(repo, 'xecute/Cargo.toml'), 'utf8');
const featureSection = cargo.split('[features]')[1].split(/\n\[/)[0];
const flags = [...featureSection.matchAll(/^([\w-]+)\s*=/gm)].map((match) => match[1]).filter((flag) => flag !== 'default');
const features = readFileSync(resolve(root, 'handbook/features.md'), 'utf8');
const tableFlags = [...features.matchAll(/^\| `([\w-]+)` \|/gm)].map((match) => match[1]);
for (const flag of flags) if (!tableFlags.includes(flag)) errors.push(`Undocumented feature: ${flag}`);
for (const flag of tableFlags) if (!flags.includes(flag)) errors.push(`Unknown feature: ${flag}`);
const constants = readFileSync(resolve(root, 'lib/constants.ts'), 'utf8');
const cliVersion = /^version = "([^"]+)"/m.exec(readFileSync(resolve(repo, 'xecute-cli/Cargo.toml'), 'utf8'))[1];
if (!constants.includes(`CURRENT_VERSION = '${cliVersion}'`)) errors.push('Documentation CLI version does not match manifest.');
function files(dir) { return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => entry.isDirectory() ? files(resolve(dir, entry.name)) : /\.tsx?$/.test(entry.name) ? [resolve(dir, entry.name)] : []); }
for (const file of files(resolve(root, 'src'))) for (const match of readFileSync(file, 'utf8').matchAll(/['"](\/docs(?:\/[a-z0-9-]+)*)['"]/g)) if (!routes.has(match[1])) errors.push(`Broken UI link: ${match[1]} in ${file}`);
if (errors.length) { console.error(errors.join('\n')); process.exitCode = 1; }
else console.log(`Validated ${routes.size} pages, navigation, source feature flags and UI routes.`);
