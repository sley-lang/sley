#include <stdio.h>
#include <stdlib.h>
#include <string.h>
typedef long long ll;
static ll fib(ll n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
static ll loopsum(ll n) { ll s = 0; for (ll i = 0; i < n; i++) s = (s + i * i) % 1000000007; return s; }
static ll collatz(ll limit) {
    ll best = 0, best_n = 0;
    for (ll n = 1; n < limit; n++) {
        ll x = n, steps = 0;
        while (x != 1) { x = x % 2 == 0 ? x / 2 : 3 * x + 1; steps++; }
        if (steps > best) { best = steps; best_n = n; }
    }
    return best_n;
}
static ll primes(ll n) {
    ll count = 0;
    for (ll k = 2; k < n; k++) {
        int is_p = 1;
        for (ll d = 2; d * d <= k; d++) if (k % d == 0) { is_p = 0; break; }
        count += is_p;
    }
    return count;
}
static ll gcd(ll a, ll b) { while (b != 0) { ll t = a % b; a = b; b = t; } return a; }
static ll gcdsum(ll n) { ll s = 0; for (ll i = 1; i < n; i++) for (ll j = 1; j < n; j++) s += gcd(i, j); return s; }
static ll tak(ll x, ll y, ll z) { return y < x ? tak(tak(x - 1, y, z), tak(y - 1, z, x), tak(z - 1, x, y)) : z; }
static ll triples(ll n) {
    ll count = 0;
    for (ll c = 1; c <= n; c++) for (ll b = 1; b < c; b++) for (ll a = 1; a < b; a++)
        if (a * a + b * b == c * c) count++;
    return count;
}
static ll digitsum(ll n) { ll t = 0; for (ll i = 0; i < n; i++) { ll x = i; while (x > 0) { t += x % 10; x /= 10; } } return t; }
static ll isqrt_(ll n) {
    ll total = 0;
    for (ll i = 0; i < n; i++) {
        ll lo = 0, hi = i + 1;
        while (hi - lo > 1) { ll mid = (lo + hi) / 2; if (mid * mid <= i) lo = mid; else hi = mid; }
        total += lo;
    }
    return total;
}
static ll inversions(ll *xs, ll n) {
    ll count = 0;
    for (ll i = 0; i < n; i++) { ll x = xs[i]; for (ll j = i + 1; j < n; j++) if (xs[j] < x) count++; }
    return count;
}
static ll *load(const char *path, ll *n) {
    FILE *f = fopen(path, "r"); ll cap = 1024, k = 0; ll *xs = malloc(cap * sizeof(ll)); int c; ll v = 0, neg = 0, in = 0;
    while ((c = fgetc(f)) != EOF) {
        if (c == '-') neg = 1;
        else if (c >= '0' && c <= '9') { v = v * 10 + (c - '0'); in = 1; }
        else if (in) { if (k == cap) xs = realloc(xs, (cap *= 2) * sizeof(ll)); xs[k++] = neg ? -v : v; v = 0; neg = 0; in = 0; }
    }
    fclose(f); *n = k; return xs;
}
int main(int argc, char **argv) {
    const char *name = argv[1]; ll r;
    if (!strcmp(name, "inversions")) { ll n; ll *xs = load(argv[2], &n); r = inversions(xs, n); }
    else if (!strcmp(name, "tak")) r = tak(atoll(argv[2]), atoll(argv[3]), atoll(argv[4]));
    else {
        ll n = atoll(argv[2]);
        if (!strcmp(name, "fib")) r = fib(n);
        else if (!strcmp(name, "loopsum")) r = loopsum(n);
        else if (!strcmp(name, "collatz")) r = collatz(n);
        else if (!strcmp(name, "primes")) r = primes(n);
        else if (!strcmp(name, "gcdsum")) r = gcdsum(n);
        else if (!strcmp(name, "triples")) r = triples(n);
        else if (!strcmp(name, "digitsum")) r = digitsum(n);
        else if (!strcmp(name, "isqrt")) r = isqrt_(n);
        else return 2;
    }
    printf("%lld\n", r);
    return 0;
}
