def first_overdraft(start: int, xs: list[int]) -> int:
    balance = start
    for i, x in enumerate(xs):
        balance += x
        if balance < 0:
            return i
    return -1
