export function fib(n: number): number {
  if (!Number.isInteger(n) || n < 0) {
    throw new RangeError(`fib: n must be a non-negative integer, got ${n}`);
  }
  let a = 0;
  let b = 1;
  for (let i = 0; i < n; i++) {
    const next = a + b;
    a = b;
    b = next;
  }
  return a;
}
