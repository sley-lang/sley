def digitsum(n: int) -> int:
    """Return the sum of the decimal digits of every integer i with 0 <= i < n."""
    return sum(sum(int(c) for c in str(i)) for i in range(n))
