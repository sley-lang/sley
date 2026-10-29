export function countMultiples(xs: number[], k: number): number {
  if (!(k > 0) || !Number.isFinite(k)) {
    throw new Error(`k must be a positive finite number, got ${k}`);
  }
  return xs.filter((x) => x % k === 0).length;
}
