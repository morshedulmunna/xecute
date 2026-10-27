// Deterministic, uncompressed ZIP: no external archive tools or packages required.
import { posix } from 'node:path';
function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
}
function zip(files) {
  const bodies = [], directory = []; let offset = 0;
  for (const [name, text] of files) {
    const filename = Buffer.from(name), data = Buffer.from(text), crc = crc32(data);
    const header = Buffer.alloc(30);
    header.writeUInt32LE(0x04034b50); header.writeUInt16LE(20, 4);
    header.writeUInt16LE(33, 12); // 1980-01-01; stable across builds.
    header.writeUInt32LE(crc, 14); header.writeUInt32LE(data.length, 18); header.writeUInt32LE(data.length, 22); header.writeUInt16LE(filename.length, 26);
    bodies.push(header, filename, data);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50); central.writeUInt16LE(20, 4); central.writeUInt16LE(20, 6); central.writeUInt16LE(33, 14);
    central.writeUInt32LE(crc, 16); central.writeUInt32LE(data.length, 20); central.writeUInt32LE(data.length, 24); central.writeUInt16LE(filename.length, 28); central.writeUInt32LE(offset, 42);
    directory.push(central, filename); offset += header.length + filename.length + data.length;
  }
  const index = Buffer.concat(directory), end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50); end.writeUInt16LE(files.size, 8); end.writeUInt16LE(files.size, 10); end.writeUInt32LE(index.length, 12); end.writeUInt32LE(offset, 16);
  return Buffer.concat([...bodies, index, end]);
}
export function agentAssets(manifest, pages, skill) {
  const result = new Map(), bundle = new Map();
  const pathFor = route => `references/${route || 'index'}.md`;
  const routes = new Map(manifest.pages.map(page => [`/docs${page.route ? `/${page.route}` : ''}`, pathFor(page.route)]));
  function localLinks(markdown, file) {
    return markdown.replace(/\]\((\/docs(?:\/[a-z0-9-]+)*)(#[^)]*)?\)/g, (match, route, hash = '') => routes.has(route) ? `](${posix.relative(posix.dirname(file), routes.get(route))}${hash})` : match);
  }
  bundle.set('SKILL.md', skill);
  let index = '# Xecute reference index\n\nRead by task, or use [the complete manual](full.md).\n\n';
  for (const category of manifest.categories) {
    index += `## ${category.title}\n\n`;
    for (const slug of category.pages) {
      const page = manifest.pages.find(page => page.route === `${category.slug}/${slug}`);
      index += `- [${page.title}](${page.route}.md): ${page.description}\n`;
    }
    index += '\n';
  }
  bundle.set('references/topics.md', index);
  let full = '# Xecute complete documentation\n\n';
  let llms = '# Xecute\n\n> Architecture tools and macros for Axum applications.\n\n## Agent resources\n\n- [Skill](skills/xecute/SKILL.md)\n- [Offline skill bundle](skills/xecute.zip)\n- [Complete manual](llms-full.txt)\n\n## Documentation\n\n';
  for (const page of manifest.pages) {
    const file = pathFor(page.route), body = pages.get(page.route);
    bundle.set(file, localLinks(body, file));
    full += `\n---\n\n# ${page.title}\n\nDocumentation route: /docs${page.route ? `/${page.route}` : ''}\n\n${localLinks(body, 'references/full.md')}\n`;
    llms += `- [${page.title}](skills/xecute/${file}): ${page.description}\n`;
  }
  bundle.set('references/full.md', full);
  for (const [file, content] of bundle) result.set(`skills/xecute/${file}`, content);
  result.set('llms.txt', llms); result.set('llms-full.txt', full.replace(/\]\((?!https?:|mailto:|#|\/)([^)]+)\)/g, '](' + 'skills/xecute/references/$1)'));
  result.set('skills/xecute.zip', zip(new Map([...bundle].map(([file, content]) => [`xecute/${file}`, content]))));
  return result;
}
