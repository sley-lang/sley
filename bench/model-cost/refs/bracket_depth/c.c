#include <stddef.h>

int bracket_depth(const char *s, long long *out)
{
    char stack[201];
    size_t top = 0;
    long long max_depth = 0;

    for (size_t i = 0; s[i] != '\0'; i++) {
        char c = s[i];
        if (c == '(' || c == '[' || c == '{') {
            stack[top++] = c;
            if ((long long)top > max_depth)
                max_depth = (long long)top;
        } else if (c == ')' || c == ']' || c == '}') {
            char open = c == ')' ? '(' : (c == ']' ? '[' : '{');
            if (top == 0 || stack[top - 1] != open)
                return 1;
            top--;
        }
    }
    if (top != 0)
        return 1;
    *out = max_depth;
    return 0;
}
