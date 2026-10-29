export function collatz(limit: number): number {
  if (limit <= 1) {
    return 0;
  }

  // cache[k] holds the number of steps for k to reach 1, for every k < limit.
  // Index 1 is 0 by default, which is the base case.
  const cache = new Uint32Array(limit);

  let best = 1;
  let bestSteps = 0;

  for (let n = 2; n < limit; n++) {
    let x = n;
    let steps = 0;

    // Walk until we drop below n, where the answer is already cached.
    while (x >= n) {
      x = x % 2 === 0 ? x / 2 : 3 * x + 1;
      steps++;
    }

    steps += cache[x];
    cache[n] = steps;

    // Strictly greater keeps the smallest n on ties.
    if (steps > bestSteps) {
      bestSteps = steps;
      best = n;
    }
  }

  return best;
}
