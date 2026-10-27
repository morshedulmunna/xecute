import { useEffect, useRef, useState, type ReactNode } from 'react';
import { ArrowRight, Check, ChevronDown, Code2, Package, Play, Pause, Terminal } from 'lucide-react';
import { Link } from 'react-router-dom';

const steps = [
  { title: 'Create', icon: Terminal, command: 'xecute new my-api', heading: 'A clear starting point.', text: 'Start with MVC, or choose CQRS. Your project arrives with an example API, configuration, Swagger and tests.', files: ['src/app_module.rs', 'src/modules/example/', 'config/AppConfig.toml'], link: '/docs/getting-started/quickstart' },
  { title: 'Generate', icon: Package, command: 'xecute g resource user', heading: 'One feature, connected.', text: 'Generate a controller, DTO, service or handlers, repository and HTTP tests. Module and route registration are handled for you.', files: ['user/controller.rs', 'user/dto/mod.rs', 'user/repository/store.rs'], link: '/docs/getting-started/first-feature' },
  { title: 'Make it yours', icon: Code2, command: 'cargo test user', heading: 'Build the part that matters.', text: 'Add your rules, test the route and connect the integrations you need. Generated repositories start in memory; persistence is your next choice.', files: ['user/service.rs', 'user/tests/http.rs', 'user/module.rs'], link: '/docs/testing/modules' },
];
const questions = [
  { question: 'Do I need a database to get started?', answer: 'No. Generated resources use an in-memory repository. Add PostgreSQL when records need to survive a restart.', link: '/docs/integrations/postgres' },
  { question: 'Should I choose MVC or CQRS?', answer: 'Start with MVC if you prefer a service for application operations. Choose CQRS when you want separate handlers for reads and writes. Both share the same module, DTO and repository ideas.', link: '/docs/architecture/mvc-cqrs' },
  { question: 'Can I add Xecute to an existing application?', answer: 'Yes. Add capabilities one at a time with explicit provider registration and configuration. Updating the CLI does not rewrite your existing application.', link: '/docs/getting-started/adopting-existing' },
];

export function LandingBottom({ children }: { children: ReactNode }) {
  const root = useRef<HTMLDivElement>(null);
  const [paused, setPaused] = useState(false);
  useEffect(() => {
    const element = root.current;
    if (!element || !('IntersectionObserver' in window)) return;
    const observer = new IntersectionObserver(entries => {
      entries.forEach(entry => {
        if (entry.isIntersecting) {
          entry.target.classList.add('x-revealed');
          observer.unobserve(entry.target);
        }
      });
    }, { threshold: 0.08 });
    element.querySelectorAll('.x-guides, .x-journey, .x-feature-explorer, .x-faq, .x-final-cta').forEach(section => {
      section.classList.add('x-reveal');
      observer.observe(section);
    });
    return () => observer.disconnect();
  }, []);
  return <div ref={root} className={`x-lower-content${paused ? ' x-bottom-paused' : ''}`}><div className="x-effects-control"><span>EXPLORE WHAT COMES NEXT</span><button type="button" aria-pressed={paused} onClick={() => setPaused(!paused)}>{paused ? <Play size={12} /> : <Pause size={12} />}{paused ? 'Play effects' : 'Pause effects'}</button></div>{children}</div>;
}

export function LandingJourney() {
  const [selected, setSelected] = useState(0);
  const step = steps[selected];
  return <section className="x-journey" aria-labelledby="journey-title"><div className="x-section-heading"><div><p className="x-kicker">A SMALL WORKFLOW. A SOLID FOUNDATION.</p><h2 id="journey-title">From idea to endpoint<span>.</span></h2></div><p>Three steps. Your own application.</p></div><div className="x-journey-layout"><div className="x-journey-steps" role="group" aria-label="Explore the development workflow">{steps.map(({ title, icon: Icon }, index) => <button type="button" key={title} aria-pressed={selected === index} aria-controls="journey-preview" onClick={() => setSelected(index)}><span className="x-step-number">0{index + 1}</span><Icon size={17} /><span>{title}</span><ArrowRight size={15} /></button>)}</div><div id="journey-preview" className="x-journey-preview" aria-live="polite"><div className="x-journey-preview-content" key={selected}><div className="x-journey-terminal"><Terminal size={14} /><code>{step.command}</code><span className="x-journey-caret" aria-hidden="true" /></div><h3>{step.heading}</h3><p>{step.text}</p><div className="x-journey-files">{step.files.map((file, index) => <span key={file} style={{ animationDelay: `${index * 90}ms` }}><Check size={11} />{file}</span>)}</div><Link to={step.link}>Follow the guide<ArrowRight size={14} /></Link></div></div></div></section>;
}

export function LandingQuestions() {
  return <section className="x-faq" aria-labelledby="faq-title"><div><p className="x-kicker">BEFORE YOU BEGIN</p><h2 id="faq-title">A few good questions.</h2><p>Small answers.<br />A clearer next step.</p></div><div>{questions.map(({ question, answer, link }) => <details key={question}><summary>{question}<ChevronDown size={16} /></summary><div className="x-faq-answer"><p>{answer}</p><Link to={link}>Read the guide<ArrowRight size={13} /></Link></div></details>)}</div></section>;
}
