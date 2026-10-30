export function collatz(limit: number): number {
  if (limit <= 1) {
    return 0;
  }

  // steps[k] holds the number of Collatz steps for k, for all k < limit.
  const steps = new Uint32Array(limit);
  steps[1] = 0;

  let bestN = 1;
  let bestSteps = 0;

  for (let n = 2; n < limit; n++) {
    let x = n;
    let count = 0;
    // Walk until the value drops below n, where the result is already cached.
    while (x >= n) {
      x = x % 2 === 0 ? x / 2 : 3 * x + 1;
      count++;
    }
    count += steps[x];
    steps[n] = count;

    if (count > bestSteps) {
      bestSteps = count;
      bestN = n;
    }
  }

  return bestN;
}
