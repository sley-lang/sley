export function collatz(limit: number): number {
  if (limit <= 1) {
    return 0;
  }

  // cache[n] = number of steps for n to reach 1 (0 for n = 1)
  const cache = new Map<number, number>();
  cache.set(1, 0);

  const steps = (start: number): number => {
    const path: number[] = [];
    let x = start;
    while (!cache.has(x)) {
      path.push(x);
      x = x % 2 === 0 ? x / 2 : 3 * x + 1;
    }
    let s = cache.get(x) as number;
    for (let i = path.length - 1; i >= 0; i--) {
      s += 1;
      cache.set(path[i], s);
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
