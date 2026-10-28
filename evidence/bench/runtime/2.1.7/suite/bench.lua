local function fib(n) if n < 2 then return n end return fib(n-1) + fib(n-2) end
local function loopsum(n) local s = 0 for i = 0, n-1 do s = (s + i*i) % 1000000007 end return s end
local function collatz(limit)
  local best, best_n = 0, 0
  for n = 1, limit-1 do
    local x, steps = n, 0
    while x ~= 1 do
      if x % 2 == 0 then x = (x) // 2 else x = 3*x + 1 end
      steps = steps + 1
    end
    if steps > best then best = steps; best_n = n end
  end
  return best_n
end
local function primes(n)
  local count = 0
  for k = 2, n-1 do
    local d, is_p = 2, true
    while d*d <= k do
      if k % d == 0 then is_p = false break end
      d = d + 1
    end
    if is_p then count = count + 1 end
  end
  return count
end
local function gcd(a, b) while b ~= 0 do a, b = b, a % b end return a end
local function gcdsum(n) local s = 0 for i = 1, n-1 do for j = 1, n-1 do s = s + gcd(i, j) end end return s end
local function tak(x, y, z)
  if y < x then return tak(tak(x-1, y, z), tak(y-1, z, x), tak(z-1, x, y)) end
  return z
end
local function triples(n)
  local count = 0
  for c = 1, n do for b = 1, c-1 do for a = 1, b-1 do
    if a*a + b*b == c*c then count = count + 1 end
  end end end
  return count
end
local function digitsum(n)
  local total = 0
  for i = 0, n-1 do
    local x = i
    while x > 0 do total = total + x % 10; x = (x) // 10 end
  end
  return total
end
local function isqrt(n)
  local total = 0
  for i = 0, n-1 do
    local lo, hi = 0, i + 1
    while hi - lo > 1 do
      local mid = (lo + hi) // 2
      if mid*mid <= i then lo = mid else hi = mid end
    end
    total = total + lo
  end
  return total
end
local function inversions(xs)
  local count, n = 0, #xs
  for i = 1, n do
    local x = xs[i]
    for j = i+1, n do if xs[j] < x then count = count + 1 end end
  end
  return count
end
local function load(path)
  local f = io.open(path); local s = f:read("a"); f:close()
  local xs = {}
  for v in s:gmatch("-?%d+") do xs[#xs+1] = tonumber(v) end
  return xs
end
local fns = {fib=fib, loopsum=loopsum, collatz=collatz, primes=primes, gcdsum=gcdsum,
  tak=tak, triples=triples, digitsum=digitsum, isqrt=isqrt, inversions=inversions}
local args = {}
for k = 2, #arg do
  if arg[k]:match("%.json$") then args[#args+1] = load(arg[k]) else args[#args+1] = tonumber(arg[k]) end
end
local unpack = table.unpack or unpack
print(fns[arg[1]](unpack(args)))
