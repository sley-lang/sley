export function primes(n: number): number {
  let count = 0;
  for (let k = 2; k < n; k++) {
    let isP = true;
    for (let d = 2; d * d <= k; d++) {
      if (k % d === 0) {
        isP = false;
        break;
      }
    }
    if (isP) {
      count++;
    }
  }
  return count;
}
