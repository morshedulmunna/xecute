import type { ComponentProps } from 'react';
import defaultMdxComponents from 'fumadocs-ui/mdx';

export default function DocumentationLink(props: ComponentProps<'a'>) {
  const href = props.href;
  if (href && (href.startsWith('/skills/') || href === '/llms.txt' || href === '/llms-full.txt')) {
    return <a {...props} href={`${import.meta.env.BASE_URL}${href.slice(1)}`} download={href.endsWith('.zip') || undefined} />;
  }
  const Anchor = defaultMdxComponents.a ?? 'a';
  return <Anchor {...props} />;
}

