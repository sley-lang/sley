export function maxSubarray(xs: number[]): number {
  if (!Array.isArray(xs) || xs.length === 0) {
    throw new Error("maxSubarray: input must be a non-empty array");
  }
  let best = xs[0];
  let current = xs[0];
  for (let i = 1; i < xs.length; i++) {
    current = Math.max(xs[i], current + xs[i]);
    if (current > best) {
      best = current;
    }
  }
  return best;
}
