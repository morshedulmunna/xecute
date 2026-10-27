import { useEffect, useState, type KeyboardEvent } from 'react';
import { Link } from 'react-router-dom';
import { ArrowRight, BookOpen, Check, Copy, Database, Github, Layers3, Network, Package, ShieldCheck, Sparkles, Terminal, Workflow, Zap } from 'lucide-react';
import { CURRENT_VERSION, GITHUB_URL } from '@/lib/constants';
import './home.css';
import { LandingBottom, LandingJourney, LandingQuestions } from './LandingBottom';

const guides = [
  { title: 'Your first application', description: 'From one command to a working API. Choose MVC or CQRS and start building.', to: '/docs/getting-started/quickstart', icon: Terminal, label: 'START BUILDING' },
  { title: 'A place for everything', description: 'Understand modules, provider scopes and the boundaries between your features.', to: '/docs/architecture/modules', icon: Layers3, label: 'THE ARCHITECTURE' },
  { title: 'Bring your existing app', description: 'Add capabilities step by step, with explicit configuration and lifecycle ownership.', to: '/docs/getting-started/adopting-existing', icon: Workflow, label: 'MAKE IT YOURS' },
  { title: 'Protect what you build', description: 'Trusted JWT verification, verified users and clear role and permission policies.', to: '/docs/integrations/auth', icon: ShieldCheck, label: 'AUTHENTICATION' },
];
const capabilities = [
  { label: 'Lifecycle', to: '/docs/architecture/lifecycle', icon: Zap, title: 'Ready before the first request.', description: 'Connect resources in dependency order. Drain work and close them when your application shuts down.', setup: 'Built into the runtime', code: 'container.initialize_async().await?;\n// Serve requests, then drain work.\ncontainer.shutdown_async().await?;', tags: ['Async startup', 'Ordered cleanup'] },
  { label: 'PostgreSQL', to: '/docs/integrations/postgres', icon: Database, title: 'Give your data a lasting home.', description: 'Move beyond in-memory storage with shared SQLx pools, migrations and transactions behind your repository.', setup: 'Cargo feature: postgres', code: 'xecute = { version = "0.1",\n  features = ["postgres"] }', tags: ['SQLx pools', 'Migrations', 'Transactions'] },
  { label: 'JWT & guards', to: '/docs/integrations/auth', icon: ShieldCheck, title: 'Put a boundary around your API.', description: 'Verify identity, then apply guards at module, controller or route level. Keep permissions explicit.', setup: 'Cargo feature: auth', code: '#[controller("/users")]\n#[guard(auth::authenticate_injected)]\n#[roles("admin")]\n#[derive(Injectable)]\npub struct UserController {}', tags: ['Verified JWTs', 'Roles & permissions'] },
  { label: 'Background jobs', to: '/docs/integrations/jobs', icon: Workflow, title: 'Let the request move on.', description: 'Run bounded in-process workers with retries and graceful draining. Use a durable broker for work that must survive a restart.', setup: 'Cargo feature: jobs', code: 'xecute = { version = "0.1",\n  features = ["jobs"] }', tags: ['Bounded workers', 'Retry policies', 'Scheduling'] },
  { label: 'TTL caching', to: '/docs/integrations/cache', icon: Package, title: 'Keep useful data close.', description: 'Choose memory or Redis, set an expiration and invalidate deliberately. You control the keys and what gets cached.', setup: 'Cargo feature: cache or cache-redis', code: 'xecute = { version = "0.1",\n  features = ["cache-redis"] }', tags: ['Memory or Redis', 'Explicit TTLs'] },
  { label: 'CQRS buses', to: '/docs/integrations/cqrs', icon: Network, title: 'One operation. A clear path.', description: 'Dispatch typed commands and queries, publish in-process events and share execution policies through behaviors.', setup: 'Cargo feature: cqrs', code: 'xecute = { version = "0.1",\n  features = ["cqrs"] }', tags: ['Typed dispatch', 'Event subscribers', 'Pipelines'] },
];
const install = 'cargo install xecute-cli';

function Brand() {
  return <span className="x-brand"><img className="x-brand-mark" src={`${import.meta.env.BASE_URL}logo.svg`} alt="" width="32" height="32" /><span>Xecute<span className="x-amber">CLI</span></span></span>;
}

export default function HomePage() {
  const [selectedCapability, setSelectedCapability] = useState(0);
  const capability = capabilities[selectedCapability];
  const architecture = 'mvc';
  const [copyState, setCopyState] = useState<'idle' | 'copied' | 'failed'>('idle');
  useEffect(() => { document.title = 'Xecute — Architecture tools for Axum'; }, []);
  useEffect(() => {
    if (copyState === 'idle') return;
    const timeout = window.setTimeout(() => setCopyState('idle'), 2500);
    return () => window.clearTimeout(timeout);
  }, [copyState]);
  function changeCapability(event: KeyboardEvent<HTMLDivElement>) {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? capabilities.length - 1 : (selectedCapability + (event.key === 'ArrowRight' ? 1 : -1) + capabilities.length) % capabilities.length;
    setSelectedCapability(next);
    document.getElementById(`capability-tab-${next}`)?.focus();
  }
  async function copyCommand() {
    try { await navigator.clipboard.writeText(install); setCopyState('copied'); }
    catch { setCopyState('failed'); }
  }

  return (
    <div className="x-landing">
      <div className="x-ambient" aria-hidden="true" />
      <header className="x-header">
        <Link to="/" aria-label="Xecute home"><Brand /></Link>
        <nav aria-label="Main navigation"><Link to="/docs">Documentation</Link><Link to="/docs/skills/ai-agent">Skills</Link><Link to="/docs/testing/examples">Examples</Link><a href={GITHUB_URL} className="x-source-link"><Github size={16} /><span>GitHub</span><ArrowRight size={14} /></a></nav>
      </header>

      <main>
        <section className="x-hero" aria-labelledby="hero-title">
          <div className="x-hero-grid">
          <div className="x-hero-copy">
            <Link to="/docs/getting-started/status" className="x-eyebrow"><span><Sparkles size={16} /></span>Built on Axum. Structured for you.<ArrowRight size={13} /></Link>
            <h1 id="hero-title">Less setup.<br />More <span>Rust.</span></h1>
            <p>Give your Axum API a clear architecture from the first command. Generate modules, wire dependencies, and keep every feature in its place.</p>
            <div className="x-hero-actions"><Link className="x-primary-action" to="/docs/getting-started/quickstart">Start building<ArrowRight size={17} /></Link><Link className="x-secondary-action" to="/docs/architecture/mvc-cqrs">Explore the architecture<BookOpen size={16} /></Link></div>
            <div className="x-hero-facts"><span><Check size={12} />Built on Axum</span><span><Check size={12} />MVC or CQRS</span><span><Check size={12} />Explicit by design</span></div>
          </div>

          <div className="x-motion-scene x-visual-preview">
            <div className="x-motion-controls"><span>YOUR FEATURE, CONNECTED</span><span className="x-preview-label">MODULAR BY DESIGN</span></div>
            <div>
              <svg className="x-motion-svg" viewBox="0 0 560 460" role="img" aria-labelledby="motion-title motion-description">
                <title id="motion-title">From a CLI command to a connected Rust feature</title>
                <desc id="motion-description">Xecute generates a user module. Requests flow through a controller, {architecture === 'mvc' ? 'service' : 'command or query handler'}, and repository. DTOs and dependency injection connect the feature.</desc>
                <defs><radialGradient id="motion-glow"><stop stopColor="#eab040" stopOpacity=".15" /><stop offset="1" stopColor="#eab040" stopOpacity="0" /></radialGradient><linearGradient id="motion-core" x2="1" y2="1"><stop stopColor="#3c301d" /><stop offset="1" stopColor="#191a1c" /></linearGradient></defs>
                <circle cx="280" cy="232" r="220" fill="url(#motion-glow)" />
                <g className="x-motion-orbit"><circle cx="280" cy="232" r="192" /><circle cx="280" cy="232" r="151" /><circle cx="472" cy="232" r="3" className="x-motion-orbit-light" /></g>
                <g className="x-motion-wires"><path d="M280 91V167M218 213H111V284M342 213H449V284M171 316H219M341 316H389M280 259V286" /></g>
                <g className="x-motion-current"><path d="M280 91V167M218 213H111V284M342 213H449V284M171 316H219M341 316H389M280 259V286" /></g>
                <g className="x-motion-command"><rect x="139" y="47" width="282" height="44" rx="10" /><text x="157" y="74" className="x-svg-prompt">❯</text><text x="177" y="74">xecute g resource user</text><rect className="x-motion-cursor" x="388" y="61" width="6" height="15" rx="1" /></g>
                <g className="x-motion-core"><rect x="218" y="167" width="124" height="92" rx="23" fill="url(#motion-core)" /><path d="m257 183 17 21-17 21h13l17-21-17-21zm30 0-8 10 8 11 17-21zm0 21-8 10 8 11h17z" fill="#ffc14a" stroke="none" /><text x="280" y="245" textAnchor="middle">user module</text></g>
                <g className="x-motion-node x-motion-node-one"><rect x="51" y="284" width="120" height="64" rx="12" /><text x="111" y="310" textAnchor="middle">Controller</text><text x="111" y="330" textAnchor="middle" className="x-svg-muted">HTTP boundary</text></g>
                <g className="x-motion-node x-motion-node-two"><rect x="219" y="284" width="122" height="64" rx="12" /><text x="280" y="310" textAnchor="middle">{architecture === 'mvc' ? 'Service' : 'Command / Query'}</text><text x="280" y="330" textAnchor="middle" className="x-svg-muted">{architecture === 'mvc' ? 'Application logic' : 'Focused handlers'}</text></g>
                <g className="x-motion-node x-motion-node-three"><rect x="389" y="284" width="120" height="64" rx="12" /><text x="449" y="310" textAnchor="middle">Repository</text><text x="449" y="330" textAnchor="middle" className="x-svg-muted">Data boundary</text></g>
                <g className="x-motion-chips"><rect x="153" y="389" width="103" height="29" rx="14" /><text x="204" y="408" textAnchor="middle">Typed DTOs</text><rect x="266" y="389" width="141" height="29" rx="14" /><text x="336" y="408" textAnchor="middle">Dependency injection</text></g>
              </svg>
            </div>
            <div className="x-preview-caption"><span className="x-status-dot" />One feature. Every layer in its place.</div>
          </div>
          </div>
          <div className={`x-hero-install x-install-${copyState}`}>
            <div className="x-install-heading"><div><span className="x-install-kicker"><Terminal size={13} /> YOUR NEXT PROJECT STARTS HERE</span><h2>A good start is<br />one command away<span>.</span></h2><p>Scaffold the structure.<br />Spend your time on the application.</p></div></div>
            <div className="x-install-console">
              <div className="x-install-console-top"><span><span className="x-window-dots" aria-hidden="true"><i /><i /><i /></span>Terminal</span><span>CLI <b>v{CURRENT_VERSION}</b></span></div>
              <div className="x-command"><span className="x-command-prompt" aria-hidden="true">❯</span><code>{install}</code><button type="button" className="x-copy-button" onClick={copyCommand} aria-label={copyState === 'copied' ? 'Command copied' : 'Copy install command'}><span className="x-copy-icon" key={copyState}>{copyState === 'copied' ? <Check size={17} /> : <Copy size={17} />}</span><span>{copyState === 'copied' ? 'Copied!' : 'Copy command'}</span><span className="x-copy-ripple" aria-hidden="true" /></button></div>
              <div className="x-panel-bottom"><span className="x-copy-feedback" role="status">{copyState === 'copied' ? 'Copied. Your next API is a paste away.' : copyState === 'failed' ? 'Copy unavailable — select the command above.' : 'Rust 1.98.1+ · Cargo'}</span><Link to="/docs/getting-started/quickstart">Let’s build<ArrowRight size={16} /></Link></div>
            </div>
          </div>
          <div className="x-hero-links"><Link to="/docs"><BookOpen size={16} />Explore the documentation<ArrowRight size={15} /></Link><span aria-hidden="true">✦</span><a href={GITHUB_URL}><Github size={16} />View the source</a></div>
        </section>

        <LandingBottom>
        <LandingJourney />
        <section className="x-guides" aria-labelledby="guides-title">
          <div className="x-section-heading"><div><p className="x-kicker">FROM FIRST ROUTE TO WHAT’S NEXT</p><h2 id="guides-title">A little structure.<br /><span>A lot of possibility.</span></h2></div><p>Build one feature at a time.<br />There’s a guide for the next step.</p></div>
          <div className="x-guide-grid">{guides.map(({ title, description, to, icon: Icon, label }) => <Link key={to} to={to} className="x-guide"><div className="x-guide-top"><Icon size={22} /><ArrowRight size={18} /></div><p className="x-kicker">{label}</p><h3>{title}</h3><p>{description}</p></Link>)}</div>
        </section>

        <section className="x-runtime x-feature-explorer" aria-labelledby="runtime-title">
          <div className="x-section-heading"><div><p className="x-kicker">SMALL CORE. ROOM TO GROW.</p><h2 id="runtime-title">Your API.<br /><span>Your building blocks.</span></h2></div><p>Start lean. Add what you need.<br />Explore a capability to see where it fits.</p></div>
          <div className="x-feature-tabs" role="tablist" aria-label="Runtime capabilities" onKeyDown={changeCapability}>{capabilities.map(({ label, icon: Icon }, index) => <button type="button" key={label} id={`capability-tab-${index}`} role="tab" aria-selected={selectedCapability === index} aria-controls="capability-panel" tabIndex={selectedCapability === index ? 0 : -1} onClick={() => setSelectedCapability(index)}><Icon size={16} />{label}</button>)}</div>
          <div id="capability-panel" className="x-feature-panel" role="tabpanel" aria-labelledby={`capability-tab-${selectedCapability}`} tabIndex={0}>
            <div className="x-feature-detail" key={capability.label}><span className="x-feature-symbol"><capability.icon size={26} /></span><h3>{capability.title}</h3><p>{capability.description}</p><div className="x-feature-tags">{capability.tags.map(tag => <span key={tag}><Check size={11} />{tag}</span>)}</div><Link to={capability.to}>Explore {capability.label}<ArrowRight size={15} /></Link></div>
            <div className="x-feature-code"><div><span><Terminal size={13} />{capability.setup}</span><span>{selectedCapability === 0 || selectedCapability === 2 ? 'Rust fragment' : 'Cargo.toml'}</span></div><pre key={capability.label}><code>{capability.code}</code></pre><p><span className="x-status-dot" />Configure explicitly. Keep your application in control.</p></div>
          </div>
        </section>
        <LandingQuestions />
        <section className="x-final-cta"><span className="x-final-mark" aria-hidden="true"><img src={`${import.meta.env.BASE_URL}logo.svg`} alt="" width="40" height="40" /></span><p className="x-kicker">MAKE SOMETHING THAT MATTERS</p><h2>The structure is ready.<br /><span>What will you build?</span></h2><Link className="x-primary-action" to="/docs/getting-started/quickstart">Build your first API<ArrowRight size={16} /></Link><p>From your first route to your next application.</p></section>
        </LandingBottom>
      </main>
      <footer className="x-footer"><Link to="/" aria-label="Xecute home"><Brand /></Link><p>Architecture tools for Axum.<span aria-hidden="true">✦</span>Apache-2.0</p><a href={GITHUB_URL}>Built in the open<ArrowRight size={14} /></a></footer>
    </div>
  );
}
