import { StrictMode, useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Icon } from './components';
import { Explorer } from './explorer';
import { Detail } from './detail';
import './styles.css';

function route() {
  return /^#\/traces\/([a-f\d]{32})\/?$/i.exec(window.location.hash)?.[1];
}

function App() {
  const [id, setId] = useState(route);
  const [connected, setConnection] = useState<boolean | null>(null);
  const demo = document.body.dataset.mode === 'demo';
  useEffect(() => {
    const changed = () => {
      setId(route());
      window.scrollTo(0, 0);
      document.querySelector<HTMLElement>('main')?.focus();
    };
    window.addEventListener('hashchange', changed);
    return () => window.removeEventListener('hashchange', changed);
  }, []);

  return <>
    <a className="skip" href="#content" onClick={event => { event.preventDefault(); document.getElementById('content')?.focus(); }}>Skip to content</a>
    <header className="shell-heading">
      <div className="shell-inner">
        <a className="brand" href="#/" aria-label="Request X-Ray home"><span className="brand-mark"><Icon name="logo" /></span><strong>Request <span>X-Ray</span></strong></a>
        <nav aria-label="Main navigation"><a href="#/" aria-current={id ? undefined : 'page'}><Icon name="traces" /> Traces</a></nav>
        <div className="shell-status">
          {demo && <span className="demo-label" data-testid="demo-indicator">Demo · synthetic data</span>}
          <span aria-live="polite"><span className={`dot ${connected === true ? 'dot-ok' : connected === false ? 'dot-error' : ''}`} />{connected === true ? 'Local collector' : connected === false ? 'Unavailable' : 'Checking collector'}</span>
        </div>
      </div>
    </header>
    <main id="content" className="shell-inner content" tabIndex={-1}>
      {id ? <Detail key={id} id={id} setConnection={setConnection} /> : <Explorer setConnection={setConnection} />}
    </main>
  </>;
}

createRoot(document.getElementById('root')!).render(<StrictMode><App /></StrictMode>);
