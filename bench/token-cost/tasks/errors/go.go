package calc

import "errors"

var (
	ErrNotPositive  = errors.New("amount must be positive")
	ErrInsufficient = errors.New("insufficient funds")
)

func withdraw(balance, amount int64) (int64, error) {
	if amount <= 0 {
		return 0, ErrNotPositive
	}
	if amount > balance {
		return 0, ErrInsufficient
	}
	return balance - amount, nil
}
