export function collatz(limit: number): number {
  if (!(limit > 1)) {
    return 0;
  }

  const size = Math.floor(limit);
  // steps[k] = number of steps for k to reach 1, for k < size.
  const steps = new Int32Array(size);
  steps[1] = 0;

  let best = 1;
  let bestSteps = 0;

  for (let n = 2; n < size; n++) {
    // Walk until the value drops below n, where the step count is already known.
    let x = n;
    let count = 0;
    while (x >= n) {
      x = x % 2 === 0 ? x / 2 : 3 * x + 1;
      count++;
    }
    const total = count + steps[x];
    steps[n] = total;

    // Strictly greater keeps the smallest n on ties.
    if (total > bestSteps) {
      bestSteps = total;
      best = n;
    }
  }

  return best;
}
