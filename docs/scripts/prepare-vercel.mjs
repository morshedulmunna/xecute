// Package only verified static output; source docs live outside the site directory.
import { cpSync, existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
const site = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const output = resolve(site, '.vercel/output');
if (!existsSync(resolve(site, 'dist/index.html'))) throw new Error('Run the documentation build before packaging.');
rmSync(output, { recursive: true, force: true });
mkdirSync(output, { recursive: true });
cpSync(resolve(site, 'dist'), resolve(output, 'static'), { recursive: true });
writeFileSync(resolve(output, 'config.json'), JSON.stringify({
  version: 3,
  routes: [
    { src: '^/skills/xecute\\.zip$', headers: { 'Content-Disposition': 'attachment; filename="xecute.zip"', 'Content-Type': 'application/zip' }, continue: true },
    { src: '^/assets/(.*)$', headers: { 'Cache-Control': 'public, max-age=31536000, immutable' }, continue: true },
    { handle: 'filesystem' },
    { src: '^/docs(?:/.*)?$', dest: '/index.html' },
  ],
}, null, 2) + '\n');
console.log('Prepared Vercel static output with documentation routes and skill downloads.');
