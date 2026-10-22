/**
 * Site-local swizzle of the docs-system landing route. The product-site plugin still adds the route
 * and loads product.json; this component renders it as Patchback's own animated front page.
 */
import React, {useEffect, useRef, type ReactNode} from 'react';
import Layout from '@theme/Layout';
import {HtmlClassNameProvider} from '@docusaurus/theme-common';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import SignalScene from '@site/src/components/landing/SignalScene';
import {PatchGraph, PianoRoll, StepGlyph} from '@site/src/components/landing/Sections';
import styles from '@site/src/components/landing/landing.module.css';

type Landing = {
  product: {name: string; eyebrow?: string; promise: string[]; lede: string; meta?: string};
};

const STEPS = [
  {
    kind: 'listen',
    title: 'Listen',
    command: 'patchback analyze',
    text: 'Onsets, pitch, envelopes and spectra of the recording at 48 kHz, reduced to 32 features.',
  },
  {
    kind: 'estimate',
    title: 'Estimate',
    command: 'regressor',
    text: 'Eight normalized parameters: from the trained regressor, or the default patch until weights exist.',
  },
  {
    kind: 'render',
    title: 'Render',
    command: 'patchback render',
    text: 'The patch plays the note events through Patchback’s own Rust engine.',
  },
  {
    kind: 'compare',
    title: 'Compare & refine',
    command: 'patchback fit',
    text: 'Four loss terms after time alignment. Coordinate search keeps only improvements, within a render budget.',
  },
] as const;

const MILESTONES = [
  {id: 'A', text: 'Sound engine, no ML', next: true},
  {id: 'B', text: 'Dataset + ModelSpec regressor'},
  {id: 'C', text: 'Monophonic phrases'},
  {id: 'D', text: 'Topologies, modulation'},
  {id: 'E', text: 'Stems and arrangement'},
];

function useNavbarOverHero(hero: React.RefObject<HTMLElement | null>) {
  useEffect(() => {
    const element = hero.current;
    const html = document.documentElement;
    if (!element) return undefined;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) html.removeAttribute('data-pb-solid');
        else html.setAttribute('data-pb-solid', '');
      },
      {rootMargin: '-72px 0px 0px 0px', threshold: 0},
    );
    observer.observe(element.querySelector('[data-pb-sentinel]') ?? element);
    return () => {
      observer.disconnect();
      html.removeAttribute('data-pb-solid');
    };
  }, [hero]);
}

export default function ProductLandingPage({landing}: {landing: Landing}): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  const {product} = landing;
  const hero = useRef<HTMLElement>(null);
  useNavbarOverHero(hero);
  const [first, ...rest] = product.promise;

  return (
    <HtmlClassNameProvider className="pb-landing">
    <Layout title={`${product.name} — ${product.promise.join(' ')}`} description={product.lede} wrapperClassName={styles.page}>
      <main>
        <section ref={hero} className={styles.hero}>
          <div className={styles.aurora} aria-hidden="true" />
          <div className={styles.heroGrid} aria-hidden="true" />
          <div className={styles.heroInner}>
            <div className={styles.heroCopy}>
              <Link to="/docs/status" className={styles.statusPill}>
                <span className={styles.statusDot} />
                In specification · first milestone has no ML
              </Link>
              <p className={styles.eyebrow}>{product.eyebrow}</p>
              <h1 className={styles.headline}>
                <span>{first}</span> <span className={styles.headlineGlow}>{rest.join(' ')}</span>
              </h1>
              <p className={styles.tagline}>{siteConfig.tagline}</p>
              <div className={styles.actions}>
                <Link className={styles.primary} to="/docs/getting-started">
                  Get started
                  <span aria-hidden="true">→</span>
                </Link>
                <Link className={styles.secondary} to="/docs/concepts/closed-loop">
                  How it works
                </Link>
              </div>
            </div>
            <div className={styles.stage}>
              <SignalScene variant="wide" />
              <SignalScene variant="compact" />
            </div>
            {product.meta && <p className={styles.meta}>{product.meta}</p>}
          </div>
          <span data-pb-sentinel className={styles.sentinel} aria-hidden="true" />
        </section>

        <section className={styles.band}>
          <div className={styles.container}>
            <p className={styles.kicker}>The closed loop</p>
            <h2 className={styles.sectionTitle}>Listen, estimate, render, compare. Repeat.</h2>
            <p className={styles.sectionLede}>The estimate only proposes a patch. The rendering and the comparison decide.</p>
            <ol className={styles.steps}>
              {STEPS.map((step, index) => (
                <li key={step.kind} className={styles.step} data-kind={step.kind}>
                  <div className={styles.stepHead}>
                    <span className={styles.stepIndex}>{String(index + 1).padStart(2, '0')}</span>
                    <StepGlyph kind={step.kind} />
                  </div>
                  <h3>{step.title}</h3>
                  <code>{step.command}</code>
                  <p>{step.text}</p>
                </li>
              ))}
            </ol>
          </div>
        </section>

        <section className={styles.band}>
          <div className={styles.container}>
            <p className={styles.kicker}>What you get back</p>
            <h2 className={styles.sectionTitle}>A program you can open, not a waveform.</h2>
            <p className={styles.sectionLede}>
              A patch file, an events file, a rendering and a report with the loss of every candidate. Parameters are kept
              in physical units.
            </p>
            <div className={styles.outputs}>
              <figure className={styles.card}>
                <figcaption>
                  <span className={styles.cardTitle}>The patch</span>
                  <Link to="/docs/concepts/patch">Patch contract →</Link>
                </figcaption>
                <PatchGraph />
                <p className={styles.cardNote}>The Milestone A voice. Values are illustrative.</p>
              </figure>
              <figure className={styles.card}>
                <figcaption>
                  <span className={styles.cardTitle}>The note events</span>
                  <Link to="/docs/concepts/events">Events →</Link>
                </figcaption>
                <PianoRoll />
                <p className={styles.cardNote}>Timed in samples at 48 kHz. Milestone A fits one isolated note; phrases come with Milestone C.</p>
              </figure>
            </div>
          </div>
        </section>

        <section className={styles.band}>
          <div className={styles.container}>
            <div className={styles.status}>
              <div>
                <p className={styles.kicker}>Status</p>
                <h2 className={styles.sectionTitle}>Specified, not built yet.</h2>
                <p className={styles.sectionLede}>
                  The design and its ESS specification exist; there is no code yet. Milestone A comes first: WAV I/O, the
                  fixed subtractive voice, analysis, the audio loss and a bounded search, with no machine learning.
                </p>
                <Link className={styles.secondaryDark} to="/docs/status">
                  What exists today →
                </Link>
              </div>
              <ol className={styles.milestones}>
                {MILESTONES.map((m) => (
                  <li key={m.id} data-next={m.next ? '' : undefined}>
                    <span>{m.id}</span>
                    {m.text}
                    {m.next && <em>first</em>}
                  </li>
                ))}
              </ol>
            </div>
          </div>
        </section>
      </main>
    </Layout>
    </HtmlClassNameProvider>
  );
}
