import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';

export function baseOptions(): BaseLayoutProps {
    return {
        links: [{ text: 'Skills', url: '/docs/skills/ai-agent' }],
        nav: {
            title: (
                <>
                    <img src={`${import.meta.env.BASE_URL}logo.svg`} alt='' width={28} height={28} className='mr-1.5' />
                    <span className='text-base font-bold tracking-tight'>Xecute</span>

                </>
            ),
        },
    };
}
