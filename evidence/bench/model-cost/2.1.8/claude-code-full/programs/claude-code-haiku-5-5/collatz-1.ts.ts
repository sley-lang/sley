export function collatz(limit: number): number {
  if (limit <= 1) {
    return 0;
  }

  // steps[n] caches the Collatz step count for n < limit; -1 means not yet computed.
  const steps = new Int32Array(limit).fill(-1);
  steps[1] = 0;

  let best = 1;
  let bestSteps = 0;

  for (let n = 2; n < limit; n++) {
    let x = n;
    let count = 0;

    // Walk until we reach a value with a known step count below the limit.
    while (x >= limit || steps[x] < 0) {
      x = x % 2 === 0 ? x / 2 : 3 * x + 1;
      count++;
    }

    const total = count + steps[x];
    steps[n] = total;

    // Strictly greater keeps the smallest n on ties, since n ascends.
    if (total > bestSteps) {
      bestSteps = total;
      best = n;
    }
  }

  return best;
}
