#include <stddef.h>

int max_depth(const long long *codes, size_t codes_len, long long *out) {
    long long depth = 0;
    long long max = 0;
    size_t i;

    if (out == NULL) {
        return 1;
    }
    if (codes == NULL && codes_len != 0) {
        return 1;
    }

    for (i = 0; i < codes_len; i++) {
        if (codes[i] == 40) {
            depth++;
            if (depth > max) {
                max = depth;
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

    *out = max;
    return 0;
}
