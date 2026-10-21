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
const fns = {fib, loopsum, collatz, primes, gcdsum};
console.log(fns[process.argv[2]](Number(process.argv[3])));
