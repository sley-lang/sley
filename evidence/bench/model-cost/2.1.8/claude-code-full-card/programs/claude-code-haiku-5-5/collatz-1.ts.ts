export function collatz(limit: number): number {
  if (limit <= 1) {
    return 0;
  }

  const n = Math.floor(limit);
  // cache[v] holds the number of steps for v to reach 1 (0 means unknown),
  // for 1 <= v < n only.
  const cache = new Uint32Array(n);

  let best = 1;
  let bestSteps = 0;

  for (let start = 1; start < n; start++) {
    const path: number[] = [];
    let v = start;

    // Walk until we reach 1 or a value whose step count is already cached.
    while (v !== 1 && (v >= n || cache[v] === 0)) {
      path.push(v);
      v = v % 2 === 0 ? v / 2 : 3 * v + 1;
    }

    const base = v === 1 ? 0 : cache[v];
    const total = path.length + base;

    // Record step counts for the visited values that fall within the cache.
    for (let i = 0; i < path.length; i++) {
      const p = path[i];
      if (p < n) {
        cache[p] = total - i;
      }
    }

    // Strict comparison keeps the smallest starting value on ties.
    if (total > bestSteps) {
      bestSteps = total;
      best = start;
    }
  }

  return best;
}
