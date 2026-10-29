export function rootSum(n: number): number {
  // Digital root of k (k >= 1) is 1 + (k - 1) % 9.
  // Sum over 1..n computed in closed form via full cycles of 9 plus remainder.
  if (n <= 0) {
    return 0;
  }
  const fullCycles = Math.floor(n / 9);
  const remainder = n % 9;
  // A full cycle of 9 consecutive integers has digital roots 1..9, summing to 45.
  let total = fullCycles * 45;
  // Remaining integers continue the cycle from 9*fullCycles + 1, whose root is 1.
  for (let r = 1; r <= remainder; r++) {
    total += r;
  }
  return total;
}
