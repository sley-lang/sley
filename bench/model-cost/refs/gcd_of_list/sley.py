import math


def gcd_of_list(xs: list[int]) -> int:
    g = 0
    for x in xs:
        g = math.gcd(g, abs(x))
    return g
