export function maxSubarray(xs: number[]): number {
  if (!Array.isArray(xs) || xs.length === 0) {
    throw new Error("maxSubarray: input array must be non-empty");
  }
  let best = xs[0];
  let current = xs[0];
  for (let i = 1; i < xs.length; i++) {
    const x = xs[i];
    current = Math.max(x, current + x);
    if (current > best) {
      best = current;
    }
  }
  return best;
}
