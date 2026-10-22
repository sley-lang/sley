#include <stdlib.h>

struct point {
    long long x;
    long long y;
};

long long manhattan(struct point a, struct point b) {
    return llabs(a.x - b.x) + llabs(a.y - b.y);
}
