import { Github, ExternalLink } from 'lucide-react';
import { GITHUB_URL, GITHUB_OWNER, GITHUB_REPO } from '@/lib/constants';

export function GithubSidebarFooter() {
  return <a href={GITHUB_URL} target="_blank" rel="noopener noreferrer" className="mx-2 mb-2 flex items-center gap-2 rounded-lg border border-fd-border px-3 py-3 text-sm text-fd-muted-foreground hover:bg-fd-accent"><Github size={16} /><span className="flex-1 truncate">{GITHUB_OWNER}/{GITHUB_REPO}</span><ExternalLink size={14} /></a>;
}
