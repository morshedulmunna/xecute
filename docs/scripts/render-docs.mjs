// Render compiled articles, landing navigation, search and anchors without a browser.
import { existsSync } from 'node:fs';
import { createServer } from 'vite';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter } from 'react-router-dom';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const server = await createServer({ root, server: { middlewareMode: true }, appType: 'custom' });
try {
  const { source, getSearchIndex } = await server.ssrLoadModule('/lib/source.ts');
  const { getMDXComponents } = await server.ssrLoadModule('/mdx-components.tsx');
  const pages = new Map();
  for (const page of source.getPages()) {
    const html = renderToStaticMarkup(React.createElement(page.data.body, { components: getMDXComponents() }));
    if (html.length < 50 || (page.data.searchText?.length ?? 0) < 80) throw new Error(`Empty article/search text: ${page.url}`);
    pages.set(page.url, { html, anchors: new Set([...html.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1])) });
  }
  const { default: HomePage } = await server.ssrLoadModule('/src/pages/HomePage.tsx');
  const home = renderToStaticMarkup(React.createElement(MemoryRouter, null, React.createElement(HomePage)));
  function checkLink(href, origin) {
    if (!href.startsWith('/docs')) return;
    const [path, hash] = href.split('#');
    const page = pages.get(path.replace(/\/$/, ''));
    if (!page || (hash && !page.anchors.has(decodeURIComponent(hash)))) throw new Error(`Broken rendered link in ${origin}: ${href}`);
  }
  for (const [url, page] of [...pages.entries(), ['/', { html: home }]]) for (const match of page.html.matchAll(/\bhref="([^"]+)"/g)) checkLink(match[1], url);
  const skillPage = pages.get('/docs/skills/ai-agent')?.html ?? '';
  if (!/<a\b[^>]*download="xecute.zip"[^>]*>/.test(skillPage)) throw new Error('Skill bundle must render as a native download link.');
  if (!skillPage.includes(`href="${server.config.base}skills/xecute.zip"`)) throw new Error('Skill bundle must link to the site asset, not GitHub.');
  for (const match of skillPage.matchAll(/\bhref="(\/(?:skills\/[^"#]+|llms(?:-full)?\.txt))"/g)) {
    if (!existsSync(resolve(root, 'public', match[1].slice(1)))) throw new Error(`Missing downloadable skill resource: ${match[1]}`);
  }
  const linked = new Set();
  function navigation(nodes) { for (const node of nodes) { if (node.url) { checkLink(node.url, 'sidebar'); linked.add(node.url); } if (node.index) { checkLink(node.index.url, 'sidebar'); linked.add(node.index.url); } if (node.children) navigation(node.children); } }
  navigation(source.pageTree.children);
  for (const url of pages.keys()) if (!linked.has(url)) throw new Error(`Page missing from runtime sidebar: ${url}`);
  const search = getSearchIndex();
  if (!search.some((entry) => entry.kind === 'section')) throw new Error('Section search indexing is empty.');
  for (const result of search) checkLink(result.url, 'search index');
  console.log(`Rendered ${pages.size} articles and homepage; verified sidebar, search and rendered link anchors (${search.length} search entries).`);
} finally { await server.close(); }
