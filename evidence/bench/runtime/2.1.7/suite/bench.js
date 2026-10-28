const fs = require("fs");
function fib(n) { return n < 2 ? n : fib(n-1) + fib(n-2); }
function loopsum(n) { let s = 0; for (let i = 0; i < n; i++) s = (s + i*i) % 1000000007; return s; }
function collatz(limit) {
  let best = 0, bestN = 0;
  for (let n = 1; n < limit; n++) {
    let x = n, steps = 0;
    while (x !== 1) { if (x % 2 === 0) x = Math.floor(x / 2); else x = 3*x + 1; steps++; }
    if (steps > best) { best = steps; bestN = n; }
  }
  return bestN;
}
function primes(n) {
  let count = 0;
  for (let k = 2; k < n; k++) {
    let d = 2, isP = true;
    while (d*d <= k) { if (k % d === 0) { isP = false; break; } d++; }
    if (isP) count++;
  }
  return count;
}
function gcd(a, b) { while (b !== 0) { [a, b] = [b, a % b]; } return a; }
function gcdsum(n) { let s = 0; for (let i = 1; i < n; i++) for (let j = 1; j < n; j++) s += gcd(i, j); return s; }
function tak(x, y, z) { return y < x ? tak(tak(x-1, y, z), tak(y-1, z, x), tak(z-1, x, y)) : z; }
function triples(n) {
  let count = 0;
  for (let c = 1; c <= n; c++) for (let b = 1; b < c; b++) for (let a = 1; a < b; a++)
    if (a*a + b*b === c*c) count++;
  return count;
}
function digitsum(n) {
  let total = 0;
  for (let i = 0; i < n; i++) { let x = i; while (x > 0) { total += x % 10; x = Math.floor(x / 10); } }
  return total;
}
function isqrt(n) {
  let total = 0;
  for (let i = 0; i < n; i++) {
    let lo = 0, hi = i + 1;
    while (hi - lo > 1) { const mid = Math.floor((lo + hi) / 2); if (mid*mid <= i) lo = mid; else hi = mid; }
    total += lo;
  }
  return total;
}
function inversions(xs) {
  let count = 0; const n = xs.length;
  for (let i = 0; i < n; i++) { const x = xs[i]; for (let j = i + 1; j < n; j++) if (xs[j] < x) count++; }
  return count;
}
const fns = {fib, loopsum, collatz, primes, gcdsum, tak, triples, digitsum, isqrt, inversions};
const args = process.argv.slice(3).map(a => a.endsWith(".json") ? JSON.parse(fs.readFileSync(a, "utf8")) : Number(a));
console.log(fns[process.argv[2]](...args));
