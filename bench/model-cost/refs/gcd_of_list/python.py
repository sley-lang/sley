import math


def gcd_of_list(xs: list[int]) -> int:
    result = 0
    for x in xs:
        result = math.gcd(result, abs(x))
    return result
