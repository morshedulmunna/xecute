import { Outlet, useLocation } from 'react-router-dom';
import { RootProvider } from 'fumadocs-ui/provider/react-router';
import { ThemeProvider } from 'next-themes';
import { docsThemeStyles } from '@/lib/theme';
import DocsThemeCustomizer from '@/src/components/DocsThemeCustomizer';

export default function AppRoot() {
  const isDocumentation = useLocation().pathname.startsWith('/docs');
  return (
    <>
      <style>{docsThemeStyles}</style>
      <ThemeProvider attribute="class" defaultTheme="system" enableSystem>
        <RootProvider>
          <Outlet />
          {isDocumentation && <DocsThemeCustomizer />}
        </RootProvider>
      </ThemeProvider>
    </>
  );
}
