#include <stddef.h>

int max_depth(const long long *codes, size_t codes_len, long long *out) {
    long long depth = 0;
    long long best = 0;

    if (out == NULL) {
        return 1;
    }

    for (size_t i = 0; i < codes_len; i++) {
        if (codes == NULL) {
            return 1;
        }
        if (codes[i] == 40) {
            depth++;
            if (depth > best) {
                best = depth;
            }
        } else if (codes[i] == 41) {
            if (depth == 0) {
                return 1;
            }
            depth--;
        }
    }

    if (depth != 0) {
        return 1;
    }

    *out = best;
    return 0;
}
