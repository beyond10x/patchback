import React, {type ReactNode} from 'react';
import styles from './landing.module.css';

const wave = (x: number, y: number, w: number, amp: number, cycles: number, points = 48) => {
  let d = '';
  for (let i = 0; i <= points; i += 1) {
    const t = i / points;
    const v = Math.sin(t * cycles * Math.PI * 2) + 0.35 * Math.sin(t * cycles * 2 * Math.PI * 2);
    d += `${i ? 'L' : 'M'}${(x + t * w).toFixed(1)} ${(y - v * amp).toFixed(1)}`;
  }
  return d;
};

export function StepGlyph({kind}: {kind: 'listen' | 'estimate' | 'render' | 'compare'}): ReactNode {
  if (kind === 'listen')
    return (
      <svg viewBox="0 0 96 56" className={styles.stepGlyph} aria-hidden="true">
        {[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map((i) => (
          <rect
            key={i}
            x={6 + i * 8}
            y={8}
            width={5}
            height={40}
            rx={2}
            className={styles.eqBar}
            style={{animationDelay: `${(i * 137) % 900}ms`, animationDuration: `${900 + ((i * 211) % 500)}ms`}}
          />
        ))}
      </svg>
    );
  if (kind === 'estimate')
    return (
      <svg viewBox="0 0 96 56" className={styles.stepGlyph} aria-hidden="true">
        {[18, 48, 78].map((cx, i) => (
          <g key={cx}>
            <circle cx={cx} cy={28} r={12} className={styles.miniKnob} />
            <g className={styles.miniKnobPointer} style={{transformOrigin: `${cx}px 28px`, animationDelay: `${i * -0.9}s`}}>
              <line x1={cx} y1={28} x2={cx} y2={18} />
            </g>
          </g>
        ))}
      </svg>
    );
  if (kind === 'render')
    return (
      <svg viewBox="0 0 96 56" className={styles.stepGlyph} aria-hidden="true">
        <path d={wave(4, 28, 88, 14, 3)} className={styles.drawWave} pathLength={100} />
      </svg>
    );
  return (
    <svg viewBox="0 0 96 56" className={styles.stepGlyph} aria-hidden="true">
      <path d={wave(4, 28, 88, 12, 2)} className={styles.refWave} />
      <path d={wave(4, 28, 88, 12, 2)} className={styles.fitWave} style={{transformOrigin: '48px 28px'}} />
    </svg>
  );
}

type GraphNode = {id: string; label: string; value: string; x: number; y: number; w: number; color: string};

const NODES: GraphNode[] = [
  {id: 'osc', label: 'Saw osc', value: 'pitch ← note', x: 14, y: 54, w: 98, color: '#2de2e6'},
  {id: 'drive', label: 'Drive', value: '6.5 dB', x: 128, y: 54, w: 86, color: '#ff9f43'},
  {id: 'filter', label: 'Lowpass', value: '1.8 kHz · Q 2.4', x: 230, y: 54, w: 120, color: '#7c8cff'},
  {id: 'vca', label: 'VCA', value: '× env', x: 366, y: 54, w: 72, color: '#ff5c9a'},
  {id: 'out', label: 'Gain', value: '−6.0 dB', x: 454, y: 54, w: 82, color: '#ff4fd8'},
  {id: 'note', label: 'Note / gate', value: 'events', x: 14, y: 168, w: 98, color: '#b6ff4a'},
  {id: 'adsr', label: 'ADSR', value: '12 ms · 240 ms · 0.62 · 380 ms', x: 196, y: 168, w: 222, color: '#c56bff'},
];

const H = 50;

function edge(from: GraphNode, to: GraphNode, mode: 'row' | 'up' | 'side' | 'down'): string {
  if (mode === 'row') return `M${from.x + from.w} ${from.y + H / 2}L${to.x} ${to.y + H / 2}`;
  if (mode === 'up') {
    const x = from.x + from.w / 2;
    return `M${x} ${from.y}L${x} ${to.y + H}`;
  }
  if (mode === 'side') return `M${from.x + from.w} ${from.y + H / 2}L${to.x} ${to.y + H / 2}`;
  const sx = from.x + from.w * 0.8;
  const tx = to.x + to.w / 2;
  return `M${sx} ${from.y}C${sx} ${from.y - 40} ${tx} ${to.y + H + 40} ${tx} ${to.y + H}`;
}

export function PatchGraph(): ReactNode {
  const [osc, drive, filter, vca, out, note, adsr] = NODES;
  const edges = [
    {d: edge(osc, drive, 'row'), c: osc.color},
    {d: edge(drive, filter, 'row'), c: drive.color},
    {d: edge(filter, vca, 'row'), c: filter.color},
    {d: edge(vca, out, 'row'), c: vca.color},
    {d: edge(note, osc, 'up'), c: note.color},
    {d: edge(note, adsr, 'side'), c: note.color},
    {d: edge(adsr, vca, 'down'), c: adsr.color},
  ];
  return (
    <svg viewBox="0 0 550 236" className={styles.graph} role="img" aria-label="The Milestone A voice: note events drive a saw oscillator and an ADSR; the oscillator feeds drive, lowpass filter, VCA and gain; the ADSR controls the VCA.">
      {edges.map((e, i) => (
        <g key={i} style={{['--c' as string]: e.c}}>
          <path d={e.d} className={styles.graphEdge} />
          <path d={e.d} className={styles.graphPulse} />
        </g>
      ))}
      {NODES.map((n) => (
        <g key={n.id} style={{['--c' as string]: n.color}}>
          <rect x={n.x} y={n.y} width={n.w} height={H} rx={9} className={styles.graphNode} />
          <rect x={n.x} y={n.y + 8} width={3} height={H - 16} rx={1.5} fill={n.color} />
          <text x={n.x + 12} y={n.y + 21} className={styles.graphLabel}>
            {n.label}
          </text>
          <text x={n.x + 12} y={n.y + 38} className={styles.graphValue}>
            {n.value}
          </text>
        </g>
      ))}
    </svg>
  );
}

const EVENTS = [
  {start: 0, end: 21600, note: 45, velocity: 0.8},
  {start: 24000, end: 33600, note: 48, velocity: 0.65},
  {start: 36000, end: 57600, note: 52, velocity: 0.9},
  {start: 60000, end: 69600, note: 50, velocity: 0.7},
];

export function PianoRoll(): ReactNode {
  const low = 43;
  const rows = 12;
  const total = 72000;
  const x0 = 36;
  const w = 500;
  const rowH = 12;
  const black = new Set([1, 3, 6, 8, 10]);
  return (
    <div>
      <svg viewBox="0 0 550 168" className={styles.roll} role="img" aria-label="Four note events on a piano roll, timed in samples at 48 kHz.">
        {Array.from({length: rows}, (_, i) => {
          const midi = low + rows - 1 - i;
          const isBlack = black.has(midi % 12);
          return (
            <g key={midi}>
              <rect x={x0} y={8 + i * rowH} width={w} height={rowH} className={isBlack ? styles.rollRowDark : styles.rollRow} />
              <rect x={6} y={8 + i * rowH + 1} width={26} height={rowH - 2} rx={2} className={isBlack ? styles.keyBlack : styles.keyWhite} />
            </g>
          );
        })}
        {Array.from({length: 7}, (_, i) => (
          <line key={i} x1={x0 + (w * i) / 6} x2={x0 + (w * i) / 6} y1={8} y2={8 + rows * rowH} className={styles.rollBeat} />
        ))}
        {EVENTS.map((e) => {
          const y = 8 + (low + rows - 1 - e.note) * rowH;
          return (
            <rect
              key={e.start}
              x={x0 + (e.start / total) * w}
              y={y + 1.5}
              width={((e.end - e.start) / total) * w}
              height={rowH - 3}
              rx={3}
              className={styles.rollNote}
              style={{opacity: 0.45 + e.velocity * 0.55}}
            />
          );
        })}
        <g className={styles.rollPlayhead}>
          <line x1={x0} x2={x0} y1={4} y2={12 + rows * rowH} />
        </g>
        {[0, 1, 2, 3, 4, 5, 6].map((i) => (
          <text key={i} x={x0 + (w * i) / 6} y={164} className={styles.rollTick} textAnchor={i === 0 ? 'start' : i === 6 ? 'end' : 'middle'}>
            {i === 0 ? '0' : `${(i * 12000).toLocaleString('en-US')}`}
          </text>
        ))}
      </svg>
      <table className={styles.eventTable}>
        <thead>
          <tr>
            <th>start_sample</th>
            <th>end_sample</th>
            <th>midi_note</th>
            <th>velocity</th>
          </tr>
        </thead>
        <tbody>
          {EVENTS.slice(0, 3).map((e) => (
            <tr key={e.start}>
              <td>{e.start}</td>
              <td>{e.end}</td>
              <td>{e.note}</td>
              <td>{e.velocity.toFixed(2)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
