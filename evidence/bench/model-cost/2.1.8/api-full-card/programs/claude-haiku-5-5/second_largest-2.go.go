package solution

import "errors"

// SecondLargest returns the second largest distinct value in xs.
// It returns an error if xs has fewer than two distinct values.
func SecondLargest(xs []int64) (int64, error) {
	var largest, second int64
	found := 0
	for _, x := range xs {
		if found == 0 {
			largest = x
			found = 1
			continue
		}
		if x == largest {
			continue
		}
		if found == 1 {
			if x > largest {
				second = largest
				largest = x
			} else {
				second = x
			}
			found = 2
			continue
		}
		if x > largest {
			second = largest
			largest = x
		} else if x > second && x != largest {
			second = x
		}
	}
	if found < 2 {
		return 0, errors.New("fewer than two distinct values")
	}
	return second, nil
}
