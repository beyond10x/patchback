import React, {useEffect, useRef, type ReactNode} from 'react';
import clsx from 'clsx';
import styles from './landing.module.css';
import {
  DEFAULT_PATCH,
  envelopePath,
  lerpParams,
  loss,
  lossTerms,
  physical,
  randomTarget,
  rng,
  tracePath,
  voice,
  type Params,
} from './synth';

type Box = {x: number; y: number; w: number; h: number};
type Port = {x: number; y: number; nx: number; ny: number};
type Cable = {id: string; from: Port; to: Port; color: string; label?: string; slow?: boolean};
type Layout = {
  name: 'wide' | 'compact';
  width: number;
  height: number;
  reference: Box;
  output: Box;
  compare: Box;
  events?: Box;
  modules: Box[];
  cables: Cable[];
  refine: boolean;
};

const C = {
  cyan: '#2de2e6',
  orange: '#ff9f43',
  blue: '#7c8cff',
  purple: '#c56bff',
  pink: '#ff5c9a',
  magenta: '#ff4fd8',
  amber: '#ffd23f',
  lime: '#b6ff4a',
};

type ModuleSpec = {title: string; color: string; params: number[]; glyph: 'saw' | 'drive' | 'filter' | 'env' | 'gain'; note?: string};

const MODULES: ModuleSpec[] = [
  {title: 'OSC', color: C.cyan, params: [], glyph: 'saw', note: 'saw · pitch from notes'},
  {title: 'DRIVE', color: C.orange, params: [6], glyph: 'drive'},
  {title: 'FILTER', color: C.blue, params: [4, 5], glyph: 'filter'},
  {title: 'ENV', color: C.purple, params: [0, 1, 2, 3], glyph: 'env'},
  {title: 'GAIN', color: C.pink, params: [7], glyph: 'gain'},
];

const KNOB_LABEL = ['ATK', 'DEC', 'SUS', 'REL', 'CUT', 'RES', 'DRV', 'OUT'];
const MODULE_OF_PARAM = [3, 3, 3, 3, 2, 2, 1, 4];
const TERM_LABEL = ['spectral', 'envelope', 'transient', 'band'];

const right = (b: Box, dy: number): Port => ({x: b.x + b.w, y: b.y + dy, nx: 1, ny: 0});
const left = (b: Box, dy: number): Port => ({x: b.x, y: b.y + dy, nx: -1, ny: 0});
const top = (b: Box, dx = b.w / 2): Port => ({x: b.x + dx, y: b.y, nx: 0, ny: -1});
const bottom = (b: Box, dx = b.w / 2): Port => ({x: b.x + dx, y: b.y + b.h, nx: 0, ny: 1});

function wideLayout(): Layout {
  const reference = {x: 20, y: 150, w: 240, h: 200};
  const modules = [318, 458, 598, 738, 878].map((x) => ({x, y: 150, w: 120, h: 200}));
  const output = {x: 1036, y: 150, w: 224, h: 200};
  const compare = {x: 470, y: 420, w: 340, h: 108};
  const events = {x: 430, y: 24, w: 300, h: 62};
  const chain: Cable[] = modules.slice(0, -1).map((box, index) => ({
    id: `chain${index}`,
    from: right(box, 46),
    to: left(modules[index + 1], 46),
    color: MODULES[index].color,
  }));
  return {
    name: 'wide',
    width: 1280,
    height: 540,
    reference,
    output,
    compare,
    events,
    modules,
    refine: true,
    cables: [
      {id: 'analyze', from: right(reference, 46), to: left(modules[0], 46), color: C.cyan},
      ...chain,
      {id: 'render', from: right(modules[4], 46), to: left(output, 46), color: C.magenta},
      {id: 'refA', from: bottom(reference), to: left(compare, 54), color: C.cyan, slow: true},
      {id: 'outB', from: bottom(output), to: right(compare, 54), color: C.magenta, slow: true},
      {id: 'noteOsc', from: bottom(events, 40), to: top(modules[0]), color: C.lime},
      {id: 'gateEnv', from: bottom(events, 260), to: top(modules[3]), color: C.lime},
    ],
  };
}

function compactLayout(): Layout {
  const reference = {x: 16, y: 16, w: 368, h: 128};
  const modules = [
    {x: 16, y: 196, w: 112, h: 200},
    {x: 144, y: 196, w: 112, h: 200},
    {x: 272, y: 196, w: 112, h: 200},
    {x: 80, y: 440, w: 112, h: 200},
    {x: 208, y: 440, w: 112, h: 200},
  ];
  const output = {x: 16, y: 690, w: 368, h: 128};
  const compare = {x: 16, y: 862, w: 368, h: 100};
  return {
    name: 'compact',
    width: 400,
    height: 978,
    reference,
    output,
    compare,
    modules,
    refine: false,
    cables: [
      {id: 'analyze', from: bottom(reference, 56), to: top(modules[0]), color: C.cyan},
      {id: 'chain0', from: right(modules[0], 46), to: left(modules[1], 46), color: MODULES[0].color},
      {id: 'chain1', from: right(modules[1], 46), to: left(modules[2], 46), color: MODULES[1].color},
      {id: 'chain2', from: bottom(modules[2]), to: left(modules[3], 46), color: MODULES[2].color},
      {id: 'chain3', from: right(modules[3], 46), to: left(modules[4], 46), color: MODULES[3].color},
      {id: 'render', from: bottom(modules[4]), to: top(output, 248), color: C.magenta},
      {id: 'outB', from: bottom(output), to: top(compare), color: C.magenta, slow: true},
    ],
  };
}

function cablePath(cable: Pick<Cable, 'from' | 'to'>, compact = false): string {
  const {from: a, to: b} = cable;
  const distance = Math.hypot(b.x - a.x, b.y - a.y);
  const reach = compact ? Math.min(54, Math.max(26, distance * 0.35)) : Math.max(30, distance * 0.42);
  const sag = Math.min(46, 12 + distance * 0.12);
  const c1x = a.x + a.nx * reach;
  const c1y = a.y + a.ny * reach + sag;
  const c2x = b.x + b.nx * reach;
  const c2y = b.y + b.ny * reach + sag;
  return `M${a.x} ${a.y}C${c1x.toFixed(1)} ${c1y.toFixed(1)} ${c2x.toFixed(1)} ${c2y.toFixed(1)} ${b.x} ${b.y}`;
}

function arc(cx: number, cy: number, r: number, from: number, to: number): string {
  const rad = (deg: number) => ((deg - 90) * Math.PI) / 180;
  const sx = cx + r * Math.cos(rad(from));
  const sy = cy + r * Math.sin(rad(from));
  const ex = cx + r * Math.cos(rad(to));
  const ey = cy + r * Math.sin(rad(to));
  const large = to - from > 180 ? 1 : 0;
  return `M${sx.toFixed(2)} ${sy.toFixed(2)}A${r} ${r} 0 ${large} 1 ${ex.toFixed(2)} ${ey.toFixed(2)}`;
}

type KnobGeometry = {cx: number; cy: number; r: number; param: number};

function knobsFor(box: Box, params: number[]): KnobGeometry[] {
  const {x, y, w} = box;
  if (params.length === 1) return [{cx: x + w / 2, cy: y + 124, r: 17, param: params[0]}];
  if (params.length === 2)
    return params.map((param, index) => ({cx: x + w * (index ? 0.73 : 0.27), cy: y + 124, r: 14, param}));
  return params.map((param, index) => ({
    cx: x + w * (index % 2 ? 0.73 : 0.27),
    cy: y + (index < 2 ? 96 : 150),
    r: 11,
    param,
  }));
}

function filterGlyph(p: Params, b: Box): string {
  const cutoff = 0.12 + p[4] * 0.7;
  const q = 0.6 + p[5] * 3;
  let d = '';
  for (let i = 0; i <= 40; i += 1) {
    const f = i / 40;
    const r = f / cutoff;
    const mag = 1 / Math.sqrt((1 - r * r) ** 2 + (r / q) ** 2);
    const v = Math.min(1, (Math.log10(mag + 1e-3) + 1.6) / 2.4);
    d += `${i ? 'L' : 'M'}${(b.x + f * b.w).toFixed(1)} ${(b.y + b.h - v * b.h).toFixed(1)}`;
  }
  return d;
}

function driveGlyph(p: Params, b: Box): string {
  const g = 0.4 + p[6] * 4.5;
  let d = '';
  for (let i = 0; i <= 24; i += 1) {
    const u = (i / 24) * 2 - 1;
    const v = Math.tanh(g * u) / Math.tanh(g);
    d += `${i ? 'L' : 'M'}${(b.x + ((u + 1) / 2) * b.w).toFixed(1)} ${(b.y + b.h / 2 - (v * b.h) / 2).toFixed(1)}`;
  }
  return d;
}

function sawGlyph(b: Box): string {
  const step = b.w / 3;
  let d = `M${b.x} ${b.y + b.h}`;
  for (let i = 0; i < 3; i += 1) d += `L${b.x + (i + 1) * step} ${b.y}L${b.x + (i + 1) * step} ${b.y + b.h}`;
  return d;
}

const glyphBox = (box: Box): Box => ({x: box.x + 14, y: box.y + 34, w: box.w - 28, h: 30});

type Refs = Map<string, SVGElement>;

function draw(refs: Refs, layout: Layout, shown: Params, target: Params, phase: number, buffers: Float32Array[]) {
  const set = (key: string, attribute: string, value: string) => refs.get(key)?.setAttribute(attribute, value);
  const text = (key: string, value: string) => {
    const node = refs.get(key);
    if (node && node.textContent !== value) node.textContent = value;
  };
  const scaleX = (key: string, value: number) => {
    const node = refs.get(key);
    if (node) node.style.transform = `scaleX(${value.toFixed(3)})`;
  };
  const scope =(b: Box): Box => ({x: b.x + 10, y: b.y + 30, w: b.w - 20, h: b.h * 0.5});
  const env = (b: Box): Box => ({x: b.x + 10, y: b.y + 32 + b.h * 0.5, w: b.w - 20, h: b.h * 0.5 - 42});
  const count = layout.name === 'wide' ? 140 : 160;
  const cycles = layout.name === 'wide' ? 2.5 : 3.5;
  const ref = voice(target, phase, count, cycles, buffers[0]);
  const out = voice(shown, phase, count, cycles, buffers[1]);
  const rs = scope(layout.reference);
  const os = scope(layout.output);
  set('refTrace', 'd', tracePath(ref.subarray(0, count), rs.x, rs.y, rs.w, rs.h));
  set('refTraceGlow', 'd', tracePath(ref.subarray(0, count), rs.x, rs.y, rs.w, rs.h));
  set('ghostTrace', 'd', tracePath(ref.subarray(0, count), os.x, os.y, os.w, os.h));
  set('outTrace', 'd', tracePath(out.subarray(0, count), os.x, os.y, os.w, os.h));
  set('outTraceGlow', 'd', tracePath(out.subarray(0, count), os.x, os.y, os.w, os.h));
  const re = env(layout.reference);
  const oe = env(layout.output);
  set('refEnv', 'd', envelopePath(target, re.x, re.y, re.w, re.h));
  set('ghostEnv', 'd', envelopePath(target, oe.x, oe.y, oe.w, oe.h));
  set('outEnv', 'd', envelopePath(shown, oe.x, oe.y, oe.w, oe.h));

  MODULES.forEach((spec, index) => {
    const g = glyphBox(layout.modules[index]);
    if (spec.glyph === 'filter') set(`glyph${index}`, 'd', filterGlyph(shown, g));
    if (spec.glyph === 'drive') set(`glyph${index}`, 'd', driveGlyph(shown, g));
    if (spec.glyph === 'env') set(`glyph${index}`, 'd', envelopePath(shown, g.x, g.y, g.w, g.h));
    if (spec.glyph === 'gain') scaleX(`glyph${index}`, 0.2 + 0.8 * shown[7]);
    knobsFor(layout.modules[index], spec.params).forEach((knob) => {
      const value = shown[knob.param];
      const angle = -135 + 270 * value;
      set(`knobArc${knob.param}`, 'd', arc(knob.cx, knob.cy, knob.r + 3.5, -135, Math.max(-134.5, angle)));
      set(`knobPointer${knob.param}`, 'transform', `rotate(${angle.toFixed(1)} ${knob.cx} ${knob.cy})`);
      text(`knobValue${knob.param}`, physical(knob.param, value));
    });
  });

  const terms = lossTerms(shown, target);
  const total = loss(terms);
  text('loss', total.toFixed(4));
  terms.forEach((term, index) => scaleX(`term${index}`, Math.max(0.004, Math.min(1, term))));
}

function Knob({knob, color, bind}: {knob: KnobGeometry; color: string; bind: (key: string) => (node: SVGElement | null) => void}) {
  const {cx, cy, r, param} = knob;
  const small = r < 13;
  return (
    <g>
      <path d={arc(cx, cy, r + 3.5, -135, 135)} className={styles.knobTrack} />
      <path ref={bind(`knobArc${param}`)} d={arc(cx, cy, r + 3.5, -135, 0)} stroke={color} className={styles.knobArc} />
      <circle cx={cx} cy={cy} r={r} className={styles.knobBody} />
      <circle cx={cx} cy={cy} r={r - 3} className={styles.knobCap} />
      <g ref={bind(`knobPointer${param}`)}>
        <line x1={cx} y1={cy - 2} x2={cx} y2={cy - r + 2.5} stroke={color} className={styles.knobPointer} />
      </g>
      <text x={cx} y={cy + r + (small ? 12 : 14)} className={styles.knobLabel} textAnchor="middle">
        {KNOB_LABEL[param]}
      </text>
      <text ref={bind(`knobValue${param}`)} x={cx} y={cy + r + (small ? 21 : 24)} className={styles.knobValue} textAnchor="middle">
        {' '}
      </text>
    </g>
  );
}

function Panel({box, label, sub, color, children}: {box: Box; label: string; sub: string; color: string; children?: ReactNode}) {
  const {x, y, w, h} = box;
  const lines: ReactNode[] = [];
  for (let i = 1; i < 8; i += 1) lines.push(<line key={`v${i}`} x1={x + (w * i) / 8} x2={x + (w * i) / 8} y1={y + 26} y2={y + h - 6} />);
  for (let i = 1; i < 4; i += 1) lines.push(<line key={`h${i}`} x1={x + 6} x2={x + w - 6} y1={y + 26 + ((h - 32) * i) / 4} y2={y + 26 + ((h - 32) * i) / 4} />);
  return (
    <g>
      <rect x={x} y={y} width={w} height={h} rx={12} className={styles.panel} style={{['--c' as string]: color}} />
      <g className={styles.grid}>{lines}</g>
      <circle cx={x + 14} cy={y + 15} r={3.5} fill={color} className={styles.led} />
      <text x={x + 24} y={y + 19} className={styles.panelLabel} fill={color}>
        {label}
      </text>
      <text x={x + w - 12} y={y + 19} className={styles.panelSub} textAnchor="end">
        {sub}
      </text>
      {children}
    </g>
  );
}

export default function SignalScene({variant}: {variant: 'wide' | 'compact'}): ReactNode {
  const layout = variant === 'wide' ? wideLayout() : compactLayout();
  const refs = useRef<Refs>(new Map());
  const root = useRef<HTMLDivElement>(null);
  const svg = useRef<SVGSVGElement>(null);
  const binders = useRef(new Map<string, (node: SVGElement | null) => void>());
  const bind = (key: string) => {
    let binder = binders.current.get(key);
    if (!binder) {
      binder = (node) => {
        if (node) refs.current.set(key, node);
        else refs.current.delete(key);
      };
      binders.current.set(key, binder);
    }
    return binder;
  };

  useEffect(() => {
    const element = root.current;
    const svgElement = svg.current;
    if (!element || !svgElement) return undefined;
    const reduce = window.matchMedia('(prefers-reduced-motion: reduce)');
    const next = rng(20261022);
    const buffers = [new Float32Array(200), new Float32Array(200)];
    let target = randomTarget(next);
    let accepted: Params = [...DEFAULT_PATCH];
    let display: Params = [...DEFAULT_PATCH];
    let shown: Params = [...DEFAULT_PATCH];
    let step = 0.34;
    let cursor = 0;
    let renders = 0;
    let iterationEnds = 0;
    let commitAt = 0;
    let pending: {param: number; value: number; keep: boolean} | null = null;
    let holdUntil = 0;
    let phase = 0;
    let last = 0;
    let frame = 0;
    let visible = false;
    const order = [4, 6, 0, 2, 5, 1, 7, 3];
    const text = (key: string, value: string) => {
      const node = refs.current.get(key);
      if (node && node.textContent !== value) node.textContent = value;
    };
    const setActive = (module: number | null, state: 'search' | 'match' | 'idle') => {
      MODULES.forEach((_, index) => refs.current.get(`module${index}`)?.classList.toggle(styles.moduleActive, index === module));
      svgElement.dataset.state = state;
      if (layout.refine && module !== null) {
        const d = cablePath({from: top(layout.compare), to: bottom(layout.modules[module])});
        refs.current.get('refine')?.setAttribute('d', d);
        refs.current.get('refinePulse')?.setAttribute('d', d);
      }
    };

    const iterate = (now: number) => {
      if (pending) {
        if (pending.keep) accepted[pending.param] = pending.value;
        display = [...accepted];
        text('verdict', pending.keep ? 'kept: loss fell' : 'rejected: loss rose');
        refs.current.get('verdict')?.setAttribute('data-keep', String(pending.keep));
        pending = null;
      }
      const current = loss(lossTerms(accepted, target));
      if (holdUntil) {
        if (now < holdUntil) return;
        holdUntil = 0;
        target = randomTarget(next);
        accepted = [...DEFAULT_PATCH];
        display = [...DEFAULT_PATCH];
        step = 0.34;
        renders = 0;
        cursor = 0;
        text('verdict', 'new reference: default patch');
        text('renders', '0');
        setActive(null, 'idle');
        iterationEnds = now + 1100;
        return;
      }
      if (current < 0.012) {
        holdUntil = now + 2800;
        display = [...accepted];
        text('verdict', 'match: best valid patch kept');
        refs.current.get('verdict')?.setAttribute('data-keep', 'true');
        setActive(null, 'match');
        return;
      }
      let param = order[cursor % order.length];
      for (let tries = 0; tries < order.length && Math.abs(target[param] - accepted[param]) < 0.006; tries += 1) {
        cursor += 1;
        param = order[cursor % order.length];
      }
      cursor += 1;
      if (cursor % order.length === 0) step = Math.max(0.03, step * 0.62);
      const gap = target[param] - accepted[param];
      const wrong = next() < 0.22 && Math.abs(gap) > 0.05;
      const direction = wrong ? -Math.sign(gap) : Math.sign(gap);
      const size = Math.min(step, Math.abs(gap) * (wrong ? 0.6 : 0.75 + next() * 0.45));
      const value = Math.min(1, Math.max(0, accepted[param] + direction * Math.max(size, 0.01)));
      const trial = [...accepted];
      trial[param] = value;
      const keep = loss(lossTerms(trial, target)) < current;
      pending = {param, value, keep};
      display = trial;
      renders += 1;
      text('renders', String(renders));
      text('verdict', `try ${KNOB_LABEL[param]} ${direction > 0 ? '+' : '−'}${Math.abs(value - accepted[param]).toFixed(2)}`);
      refs.current.get('verdict')?.setAttribute('data-keep', '');
      setActive(MODULE_OF_PARAM[param], 'search');
      commitAt = now + 360;
    };

    const tick = (now: number) => {
      frame = 0;
      if (!visible) return;
      const dt = last ? Math.min(0.05, (now - last) / 1000) : 0.016;
      last = now;
      if (commitAt && now >= commitAt) {
        commitAt = 0;
        iterationEnds = now;
      }
      if (now >= iterationEnds) {
        iterate(now);
        if (!commitAt) iterationEnds = Math.max(iterationEnds, now + 520);
        else iterationEnds = Number.POSITIVE_INFINITY;
      }
      const ease = 1 - Math.exp(-dt * 11);
      shown = shown.map((value, index) => value + (display[index] - value) * ease);
      phase -= dt * 2.4;
      draw(refs.current, layout, shown, target, phase, buffers);
      frame = requestAnimationFrame(tick);
    };

    const still = () => {
      shown = lerpParams(DEFAULT_PATCH, target, 0.82);
      draw(refs.current, layout, shown, target, 0.6, buffers);
      text('renders', '214');
      text('verdict', 'kept: loss fell');
      setActive(2, 'search');
    };

    const start = () => {
      if (frame || reduce.matches) return;
      last = 0;
      svgElement.unpauseAnimations?.();
      frame = requestAnimationFrame(tick);
    };
    const stop = () => {
      if (frame) cancelAnimationFrame(frame);
      frame = 0;
      svgElement.pauseAnimations?.();
    };

    still();
    if (reduce.matches) svgElement.pauseAnimations?.();
    const observer = new IntersectionObserver(
      ([entry]) => {
        visible = entry.isIntersecting;
        element.classList.toggle(styles.paused, !visible || reduce.matches);
        if (visible && !reduce.matches) start();
        else stop();
      },
      {threshold: 0.02},
    );
    observer.observe(element);
    const onMotion = () => {
      if (reduce.matches) {
        stop();
        still();
        element.classList.add(styles.paused);
      } else if (visible) {
        element.classList.remove(styles.paused);
        display = [...accepted];
        start();
      }
    };
    reduce.addEventListener('change', onMotion);
    return () => {
      observer.disconnect();
      reduce.removeEventListener('change', onMotion);
      stop();
    };
    // The layout is fixed per variant.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [variant]);

  const {reference, output, compare, events} = layout;
  const compact = layout.name === 'compact';
  const firstRefine = cablePath({from: top(compare), to: bottom(layout.modules[2])});

  return (
    <div ref={root} className={clsx(styles.scene, compact ? styles.sceneCompact : styles.sceneWide)}>
      <svg
        ref={svg}
        viewBox={`0 0 ${layout.width} ${layout.height}`}
        className={styles.sceneSvg}
        role="img"
        aria-label="Animated patch: a reference waveform enters on the left, flows through oscillator, drive, filter, envelope and gain modules whose knobs turn, and the rendered waveform converges on the reference while the comparison loss falls."
        data-state="search">
        <defs>
          <linearGradient id={`fade-${variant}`} x1="0" x2="0" y1="0" y2="1">
            <stop offset="0" stopColor="#ffffff" stopOpacity="0.07" />
            <stop offset="1" stopColor="#ffffff" stopOpacity="0" />
          </linearGradient>
        </defs>

        {layout.cables.map((cable) => {
          const d = cablePath(cable, compact);
          return (
            <g key={cable.id} className={styles.cable} style={{['--c' as string]: cable.color}}>
              <path d={d} className={styles.cableGlow} />
              <path d={d} className={styles.cableCore} />
              <path d={d} className={clsx(styles.cablePulse, cable.slow && styles.cablePulseSlow)} />
              <circle r={3.2} className={styles.packet}>
                <animateMotion dur={cable.slow ? '3.2s' : '1.6s'} repeatCount="indefinite" path={d} />
              </circle>
            </g>
          );
        })}

        {layout.refine && (
          <g className={clsx(styles.cable, styles.refine)} style={{['--c' as string]: C.amber}}>
            <path ref={bind('refine')} d={firstRefine} className={styles.cableGlow} />
            <path ref={bind('refinePulse')} d={firstRefine} className={clsx(styles.cablePulse, styles.refinePulse)} />
          </g>
        )}

        {events && (
          <g>
            <rect x={events.x} y={events.y} width={events.w} height={events.h} rx={10} className={styles.panel} style={{['--c' as string]: C.lime}} />
            <text x={events.x + 12} y={events.y + 18} className={styles.panelLabel} fill={C.lime}>
              NOTE EVENTS
            </text>
            <text x={events.x + events.w - 12} y={events.y + 18} className={styles.panelSub} textAnchor="end">
              note · gate · velocity
            </text>
            {[0, 1, 2, 3, 4, 5, 6, 7].map((i) => (
              <line key={i} x1={events.x + 12} x2={events.x + events.w - 12} y1={events.y + 28 + i * 4} y2={events.y + 28 + i * 4} className={styles.rollLine} />
            ))}
            <rect x={events.x + 40} y={events.y + 36} width={150} height={6} rx={2} fill={C.lime} className={styles.note} />
            <rect x={events.x + 12} y={events.y + 26} width={2} height={32} fill={C.lime} className={styles.playhead} />
          </g>
        )}

        <Panel box={reference} label="REFERENCE" sub="48 kHz" color={C.cyan}>
          <path ref={bind('refEnv')} className={styles.envFill} stroke={C.cyan} />
          <path ref={bind('refTraceGlow')} className={styles.traceGlow} stroke={C.cyan} />
          <path ref={bind('refTrace')} className={styles.trace} stroke={C.cyan} />
        </Panel>

        <Panel box={output} label="RENDER" sub="patch → engine" color={C.magenta}>
          <path ref={bind('ghostEnv')} className={styles.ghostEnv} />
          <path ref={bind('outEnv')} className={styles.envFill} stroke={C.magenta} />
          <path ref={bind('ghostTrace')} className={styles.ghost} />
          <path ref={bind('outTraceGlow')} className={styles.traceGlow} stroke={C.magenta} />
          <path ref={bind('outTrace')} className={styles.trace} stroke={C.magenta} />
        </Panel>

        {MODULES.map((spec, index) => {
          const box = layout.modules[index];
          const g = glyphBox(box);
          return (
            <g key={spec.title} ref={bind(`module${index}`)} className={styles.module} style={{['--c' as string]: spec.color}}>
              <rect x={box.x} y={box.y} width={box.w} height={box.h} rx={12} className={styles.moduleBody} />
              <rect x={box.x} y={box.y} width={box.w} height={box.h} rx={12} fill={`url(#fade-${variant})`} />
              <rect x={box.x + 1} y={box.y + 1} width={box.w - 2} height={3} rx={1.5} fill={spec.color} className={styles.moduleStrip} />
              <circle cx={box.x + box.w - 14} cy={box.y + 17} r={3.5} className={styles.moduleLed} />
              <text x={box.x + 12} y={box.y + 22} className={styles.moduleTitle} fill={spec.color}>
                {spec.title}
              </text>
              <rect x={g.x - 4} y={g.y - 3} width={g.w + 8} height={g.h + 6} rx={5} className={styles.glyphWell} />
              {spec.glyph === 'saw' && <path d={sawGlyph(g)} className={styles.glyph} stroke={spec.color} />}
              {spec.glyph === 'gain' && (
                <g>
                  <path d={`M${g.x} ${g.y + 2}L${g.x + 22} ${g.y + g.h / 2}L${g.x} ${g.y + g.h - 2}Z`} className={styles.glyph} stroke={spec.color} />
                  <rect x={g.x + 30} y={g.y + g.h / 2 - 4} width={g.w - 30} height={8} rx={3} className={styles.meterTrack} />
                  <rect
                    ref={bind(`glyph${index}`)}
                    x={g.x + 30}
                    y={g.y + g.h / 2 - 4}
                    width={g.w - 30}
                    height={8}
                    rx={3}
                    fill={spec.color}
                    style={{transformOrigin: `${g.x + 30}px 0`, transformBox: 'view-box'}}
                  />
                </g>
              )}
              {(spec.glyph === 'filter' || spec.glyph === 'drive' || spec.glyph === 'env') && (
                <path ref={bind(`glyph${index}`)} className={styles.glyph} stroke={spec.color} />
              )}
              {spec.note && (
                <>
                  <circle cx={box.x + box.w / 2} cy={box.y + 124} r={18} className={styles.jack} />
                  <circle cx={box.x + box.w / 2} cy={box.y + 124} r={7} className={styles.jackHole} />
                  <text x={box.x + box.w / 2} y={box.y + 166} className={styles.knobLabel} textAnchor="middle">
                    {spec.note}
                  </text>
                </>
              )}
              {knobsFor(box, spec.params).map((knob) => (
                <Knob key={knob.param} knob={knob} color={spec.color} bind={bind} />
              ))}
            </g>
          );
        })}

        <g className={styles.compare}>
          <rect x={compare.x} y={compare.y} width={compare.w} height={compare.h} rx={12} className={styles.panel} style={{['--c' as string]: C.amber}} />
          <text x={compare.x + 14} y={compare.y + 22} className={styles.panelLabel} fill={C.amber}>
            COMPARE
          </text>
          <text x={compare.x + compare.w - 14} y={compare.y + 22} className={styles.panelSub} textAnchor="end">
            render <tspan ref={bind('renders')}>0</tspan> / 2000
          </text>
          <text x={compare.x + 14} y={compare.y + 56} className={styles.lossLabel}>
            L
          </text>
          <text ref={bind('loss')} x={compare.x + 30} y={compare.y + 58} className={styles.lossValue}>
            0.0000
          </text>
          <text ref={bind('verdict')} x={compare.x + 14} y={compare.y + compare.h - 14} className={styles.verdict}>
            {' '}
          </text>
          {TERM_LABEL.map((label, index) => {
            const bx = compare.x + compare.w * 0.5;
            const bw = compare.w * 0.5 - 16;
            const by = compare.y + 36 + index * 16;
            return (
              <g key={label}>
                <text x={bx} y={by + 7} className={styles.termLabel}>
                  {label}
                </text>
                <rect x={bx + 58} y={by} width={bw - 58} height={7} rx={3} className={styles.meterTrack} />
                <rect
                  ref={bind(`term${index}`)}
                  x={bx + 58}
                  y={by}
                  width={bw - 58}
                  height={7}
                  rx={3}
                  className={styles.termBar}
                  style={{transformOrigin: `${bx + 58}px 0`, transformBox: 'view-box'}}
                />
              </g>
            );
          })}
        </g>
      </svg>
    </div>
  );
}
