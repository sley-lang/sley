export function collatz(limit: number): number {
  if (limit <= 1) {
    return 0;
  }

  // cache[v] = (remaining steps to reach 1) + 1, for v < limit; 0 means unknown.
  const cache = new Uint32Array(limit);
  cache[1] = 1;

  let best = 1;
  let bestSteps = 0;

  for (let n = 2; n < limit; n++) {
    // Walk until we reach a value below limit whose step count is already known.
    const path: number[] = [];
    let x = n;
    let steps = 0;
    while (x >= limit || cache[x] === 0) {
      path.push(x);
      x = x % 2 === 0 ? x / 2 : 3 * x + 1;
      steps++;
    }

    const total = steps + (cache[x] - 1);

    // path[i] is reached after i steps, so it has (total - i) steps remaining.
    for (let i = 0; i < path.length; i++) {
      const v = path[i];
      if (v < limit) {
        cache[v] = total - i + 1;
      }
    }

    // Strictly greater keeps the smallest n on ties, since n increases.
    if (total > bestSteps) {
      bestSteps = total;
      best = n;
    }
  }

  return best;
}
