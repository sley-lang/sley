export function collatz(limit: number): number {
  if (limit <= 1) {
    return 0;
  }

  // cache[n] holds the number of steps for starting value n to reach 1.
  const cache = new Uint32Array(limit);
  cache[1] = 0;

  let best = 1;
  let bestSteps = 0;

  for (let n = 2; n < limit; n++) {
    let x = n;
    let steps = 0;
    // Walk until the sequence drops below n, whose step count is already cached.
    while (x >= n) {
      x = x % 2 === 0 ? x / 2 : 3 * x + 1;
      steps++;
    }
    steps += cache[x];
    cache[n] = steps;

    // Strict comparison keeps the smallest n on ties.
    if (steps > bestSteps) {
      bestSteps = steps;
      best = n;
    }
  }

  return best;
}
