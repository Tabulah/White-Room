import { describe, expect, it } from 'vitest';
import css from './tokens.css?raw';

/** `--color-<name>: <value>;` declarations, in order. */
const colors = new Map(
  [...css.matchAll(/^\s*--color-([a-z0-9-]+):\s*([^;]+);/gm)].map(([, name, value]) => [
    name,
    value.trim(),
  ]),
);

function resolve(name: string, seen = new Set<string>()): string {
  const value = colors.get(name);
  if (value === undefined) throw new Error(`unknown colour token --color-${name}`);
  const reference = /^var\(--color-([a-z0-9-]+)\)$/.exec(value);
  if (!reference) return value;
  if (seen.has(name)) throw new Error(`circular colour token --color-${name}`);
  return resolve(reference[1], seen.add(name));
}

function channels(hex: string): [number, number, number] {
  const match = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  if (!match) throw new Error(`expected #rrggbb, got ${hex}`);
  return [1, 2, 3].map((i) => parseInt(match[i], 16)) as [number, number, number];
}

/** WCAG 2.x relative luminance and contrast ratio. */
function luminance(hex: string): number {
  const [r, g, b] = channels(hex).map((c) => {
    const s = c / 255;
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

describe('design tokens', () => {
  it("removes Tailwind's default palette so no hue is available", () => {
    expect(css).toMatch(/--color-\*:\s*initial;/);
  });

  it('defines only neutral greys', () => {
    expect(colors.size).toBeGreaterThan(0);
    for (const name of colors.keys()) {
      const [r, g, b] = channels(resolve(name));
      expect({ name, r, g, b }).toEqual({ name, r, g: r, b: r });
    }
  });

  const surfaces = ['surface', 'surface-raised', 'surface-sunken'];
  const inks = ['ink', 'ink-muted', 'ink-faint'];

  it.each(inks.flatMap((ink) => surfaces.map((surface) => [ink, surface])))(
    'text colour %s meets WCAG AA (4.5:1) on %s',
    (ink, surface) => {
      expect(contrast(resolve(ink), resolve(surface))).toBeGreaterThanOrEqual(4.5);
    },
  );

  it('inverse text meets WCAG AA on ink', () => {
    expect(contrast(resolve('ink-inverse'), resolve('ink'))).toBeGreaterThanOrEqual(4.5);
  });

  it.each(surfaces)('focus ring meets WCAG non-text contrast (3:1) on %s', (surface) => {
    expect(contrast(resolve('focus'), resolve(surface))).toBeGreaterThanOrEqual(3);
  });

  it('resets the type scale to the named sizes', () => {
    expect(css).toMatch(/--text-\*:\s*initial;/);
    for (const size of ['micro', 'caption', 'body', 'lead', 'title', 'display']) {
      expect(css).toMatch(new RegExp(`--text-${size}:`));
    }
  });
});
