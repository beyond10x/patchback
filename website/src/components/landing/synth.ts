/**
 * Illustrative signal maths for the landing-page scene. Nothing here is Patchback's engine: it draws
 * a plausible saw voice from the eight normalized parameters of DESIGN.md § 5.2 so the picture
 * changes the way a fit would.
 */

export type Params = number[];

export const PARAM_NAMES = ['attack', 'decay', 'sustain', 'release', 'cutoff', 'resonance', 'drive', 'gain'] as const;

export const DEFAULT_PATCH: Params = [0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5];

const clamp = (value: number) => Math.min(1, Math.max(0, value));

/** Physical value of one normalized parameter, formatted, after the example mapping in § 5.2. */
export function physical(index: number, p: number): string {
  const ms = (seconds: number) => (seconds < 1 ? `${Math.round(seconds * 1000)} ms` : `${seconds.toFixed(2)} s`);
  switch (index) {
    case 0:
      return ms(0.001 * Math.pow(2000, p));
    case 1:
    case 3:
      return ms(0.005 * Math.pow(400, p));
    case 2:
      return p.toFixed(2);
    case 4: {
      const hz = 40 * Math.pow(400, p);
      return hz >= 1000 ? `${(hz / 1000).toFixed(1)} kHz` : `${Math.round(hz)} Hz`;
    }
    case 5:
      return `Q ${(0.5 + 11.5 * p).toFixed(1)}`;
    case 6:
      return `${(24 * p).toFixed(1)} dB`;
    default:
      return `${(-24 + 24 * p).toFixed(1)} dB`;
  }
}

/** A deterministic generator, so the server render and the first client frame agree. */
export function rng(seed: number) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export function randomTarget(next: () => number): Params {
  return [
    0.08 + next() * 0.4,
    0.2 + next() * 0.6,
    0.25 + next() * 0.6,
    0.2 + next() * 0.6,
    0.15 + next() * 0.55,
    0.1 + next() * 0.75,
    0.1 + next() * 0.8,
    0.55 + next() * 0.4,
  ];
}

const HARMONICS = 14;

/** One frame of the voice as `count` samples in [-1, 1], spanning `cycles` periods. */
export function voice(p: Params, phase: number, count: number, cycles: number, out: Float32Array): Float32Array {
  const cutoff = 1.1 + p[4] * 9;
  const q = 0.55 + p[5] * 3.2;
  const drive = 0.6 + p[6] * 4.2;
  const weights = new Float32Array(HARMONICS);
  for (let k = 1; k <= HARMONICS; k += 1) {
    const r = k / cutoff;
    weights[k - 1] = (1 / k) / Math.sqrt((1 - r * r) ** 2 + (r / q) ** 2);
  }
  let peak = 1e-6;
  for (let i = 0; i < count; i += 1) {
    const t = phase + (i / (count - 1)) * cycles * Math.PI * 2;
    let y = 0;
    for (let k = 1; k <= HARMONICS; k += 1) y += weights[k - 1] * Math.sin(k * t);
    y = Math.tanh(drive * y * 0.6);
    out[i] = y;
    peak = Math.max(peak, Math.abs(y));
  }
  const level = 0.35 + 0.65 * p[7];
  for (let i = 0; i < count; i += 1) out[i] = (out[i] / peak) * level;
  return out;
}

/** An SVG path through `samples`, inside the box (x, y, width, height). */
export function tracePath(samples: Float32Array, x: number, y: number, width: number, height: number): string {
  const mid = y + height / 2;
  const half = height / 2;
  const step = width / (samples.length - 1);
  let d = `M${x.toFixed(1)} ${(mid - samples[0] * half).toFixed(1)}`;
  for (let i = 1; i < samples.length; i += 1) d += `L${(x + i * step).toFixed(1)} ${(mid - samples[i] * half).toFixed(1)}`;
  return d;
}

/** The ADSR contour as an SVG path inside the box. */
export function envelopePath(p: Params, x: number, y: number, width: number, height: number): string {
  const a = 0.04 + p[0] * 0.22;
  const d = 0.05 + p[1] * 0.22;
  const r = 0.06 + p[3] * 0.26;
  const hold = Math.max(0.05, 1 - a - d - r);
  const s = 0.12 + p[2] * 0.8;
  const level = 0.35 + 0.65 * p[7];
  const px = (f: number) => (x + f * width).toFixed(1);
  const py = (v: number) => (y + height - v * level * height).toFixed(1);
  const x1 = a;
  const x2 = a + d;
  const x3 = a + d + hold;
  return `M${px(0)} ${py(0)}L${px(x1)} ${py(1)}Q${px(x1 + d * 0.25)} ${py(s)} ${px(x2)} ${py(s)}L${px(x3)} ${py(s)}Q${px(x3 + r * 0.2)} ${py(0)} ${px(1)} ${py(0)}`;
}

/** The four comparison terms of § 9.1, imitated from parameter distance, each in [0, 1]. */
export function lossTerms(p: Params, target: Params): [number, number, number, number] {
  const d = p.map((value, index) => Math.abs(value - target[index]));
  return [
    clamp(0.9 * d[4] + 0.55 * d[5] + 0.45 * d[6]),
    clamp(0.5 * d[0] + 0.45 * d[1] + 0.6 * d[2] + 0.4 * d[3]),
    clamp(1.0 * d[0] + 0.5 * d[6]),
    clamp(0.7 * d[4] + 0.9 * d[7]),
  ];
}

export const LOSS_WEIGHTS = [0.45, 0.25, 0.15, 0.15];

export function loss(terms: readonly number[]): number {
  return terms.reduce((sum, term, index) => sum + term * LOSS_WEIGHTS[index], 0);
}

export function lerpParams(from: Params, to: Params, amount: number): Params {
  return from.map((value, index) => value + (to[index] - value) * amount);
}
