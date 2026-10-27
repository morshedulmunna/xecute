import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
if (!process.env.SITE_URL) {
  console.log('SITE_URL is unset; no deployment URL or sitemap is assumed.');
  process.exit(0);
}
const base = new URL(process.env.SITE_URL);
if (!['https:', 'http:'].includes(base.protocol) || base.username || base.password || base.search || base.hash) throw new Error('SITE_URL must be a public HTTP(S) base URL without credentials, query or fragment.');
const site = base.href.replace(/\/$/, '');
const manifest = JSON.parse(readFileSync(resolve(root, 'content-manifest.json'), 'utf8'));
const urls = [`${site}/`, ...manifest.pages.map((page) => `${site}/docs${page.route ? `/${page.route}` : ''}`)];
const escape = (value) => value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');
const date = new Date().toISOString().slice(0, 10);
const xml = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls.map((url) => `  <url><loc>${escape(url)}</loc><lastmod>${date}</lastmod></url>`).join('\n')}\n</urlset>\n`;
writeFileSync(resolve(root, 'dist/sitemap.xml'), xml);
writeFileSync(resolve(root, 'dist/robots.txt'), `User-agent: *\nAllow: /\n\nSitemap: ${site}/sitemap.xml\n`);
console.log(`Generated sitemap for ${urls.length} URLs under ${site}.`);
