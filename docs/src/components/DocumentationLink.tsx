import type { ComponentProps } from 'react';
import { Download } from 'lucide-react';
import defaultMdxComponents from 'fumadocs-ui/mdx';

export default function DocumentationLink(props: ComponentProps<'a'>) {
  const href = props.href;
  if (href && (href.startsWith('/skills/') || href === '/llms.txt' || href === '/llms-full.txt')) {
    const url = `${import.meta.env.BASE_URL}${href.slice(1)}`;
    if (href.endsWith('.zip')) {
      return <a {...props} href={url} download="xecute.zip" className="not-prose my-3 inline-flex items-center gap-2 rounded-lg bg-amber-300 px-5 py-3 text-sm font-semibold text-neutral-950 no-underline transition-colors hover:bg-amber-200 focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-amber-300"><Download size={17} aria-hidden="true" />{props.children}</a>;
    }
    return <a {...props} href={url} />;
  }
  const Anchor = defaultMdxComponents.a ?? 'a';
  return <Anchor {...props} />;
}

