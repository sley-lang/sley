def fib(n) = n < 2 ? n : fib(n-1) + fib(n-2)
def loopsum(n) s = 0; i = 0; while i < n; s = (s + i*i) % 1000000007; i += 1; end; s end
def collatz(limit)
  best = 0; best_n = 0; n = 1
  while n < limit
    x = n; steps = 0
    while x != 1
      if x % 2 == 0 then x = x / 2 else x = 3*x + 1 end
      steps += 1
    end
    if steps > best then best = steps; best_n = n end
    n += 1
  end
  best_n
end
def primes(n)
  count = 0; k = 2
  while k < n
    d = 2; is_p = true
    while d*d <= k
      if k % d == 0 then is_p = false; break end
      d += 1
    end
    count += 1 if is_p
    k += 1
  end
  count
end
def gcd(a, b) while b != 0; a, b = b, a % b; end; a end
def gcdsum(n) s = 0; i = 1; while i < n; j = 1; while j < n; s += gcd(i, j); j += 1; end; i += 1; end; s end
puts send(ARGV[0], Integer(ARGV[1]))
