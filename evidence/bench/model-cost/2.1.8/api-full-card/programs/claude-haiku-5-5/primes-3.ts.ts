export function primes(n: number): number {
  if (n <= 2) {
    return 0;
  }
  const sieve: boolean[] = new Array(n).fill(true);
  sieve[0] = false;
  sieve[1] = false;
  for (let i = 2; i * i < n; i++) {
    if (sieve[i]) {
      for (let j = i * i; j < n; j += i) {
        sieve[j] = false;
      }
    }
  }
  let count = 0;
  for (let i = 0; i < n; i++) {
    if (sieve[i]) {
      count++;
    }
  }
  return count;
}
