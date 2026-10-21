local function fib(n) if n < 2 then return n end return fib(n-1) + fib(n-2) end
local function loopsum(n) local s = 0 for i = 0, n-1 do s = (s + i*i) % 1000000007 end return s end
local function collatz(limit)
  local best, best_n = 0, 0
  for n = 1, limit-1 do
    local x, steps = n, 0
    while x ~= 1 do
      if x % 2 == 0 then x = x // 2 else x = 3*x + 1 end
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
local fns = {fib=fib, loopsum=loopsum, collatz=collatz, primes=primes, gcdsum=gcdsum}
print(fns[arg[1]](math.tointeger(tonumber(arg[2])) or tonumber(arg[2])))
