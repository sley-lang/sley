export function collatz(limit: number): number {
  if (limit <= 1) {
    return 0;
  }

  // cache[v] = number of steps for v to reach 1, for v < limit; -1 if unknown.
  const cache = new Int32Array(limit).fill(-1);
  cache[1] = 0;

  const steps = (n: number): number => {
    const path: number[] = [];
    let x = n;
    while (x >= limit || cache[x] < 0) {
      path.push(x);
      x = x % 2 === 0 ? x / 2 : 3 * x + 1;
    }
    let s = cache[x];
    for (let i = path.length - 1; i >= 0; i--) {
      s += 1;
      const v = path[i];
      if (v < limit) {
        cache[v] = s;
      }
    }
    return s;
  };

  let bestN = 1;
  let bestSteps = 0;
  for (let n = 1; n < limit; n++) {
    const s = steps(n);
    if (s > bestSteps) {
      bestSteps = s;
      bestN = n;
    }
  }
  return bestN;
}
