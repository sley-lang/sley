require "json"
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
def tak(x, y, z) = y < x ? tak(tak(x-1, y, z), tak(y-1, z, x), tak(z-1, x, y)) : z
def triples(n)
  count = 0; c = 1
  while c <= n
    b = 1
    while b < c
      a = 1
      while a < b
        count += 1 if a*a + b*b == c*c
        a += 1
      end
      b += 1
    end
    c += 1
  end
  count
end
def digitsum(n)
  total = 0; i = 0
  while i < n
    x = i
    while x > 0; total += x % 10; x /= 10; end
    i += 1
  end
  total
end
def isqrt(n)
  total = 0; i = 0
  while i < n
    lo = 0; hi = i + 1
    while hi - lo > 1
      mid = (lo + hi) / 2
      if mid*mid <= i then lo = mid else hi = mid end
    end
    total += lo; i += 1
  end
  total
end
def inversions(xs)
  count = 0; n = xs.length; i = 0
  while i < n
    x = xs[i]; j = i + 1
    while j < n; count += 1 if xs[j] < x; j += 1; end
    i += 1
  end
  count
end
args = ARGV[1..].map { |a| a.end_with?(".json") ? JSON.parse(File.read(a)) : Integer(a) }
puts send(ARGV[0], *args)
