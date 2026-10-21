enum withdraw_error { WITHDRAW_OK, WITHDRAW_NOT_POSITIVE, WITHDRAW_INSUFFICIENT };

enum withdraw_error withdraw(long long balance, long long amount, long long *out) {
    if (amount <= 0)
        return WITHDRAW_NOT_POSITIVE;
    if (amount > balance)
        return WITHDRAW_INSUFFICIENT;
    *out = balance - amount;
    return WITHDRAW_OK;
}
