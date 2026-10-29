package solution

import "errors"

// SecondLargest returns the second largest distinct value in xs.
// It returns an error if xs has fewer than two distinct values.
func SecondLargest(xs []int64) (int64, error) {
	var largest, second int64
	count := 0
	for _, x := range xs {
		if count == 0 {
			largest = x
			count = 1
			continue
		}
		if x == largest {
			continue
		}
		if count == 1 {
			// first distinct value different from largest
			if x > largest {
				second = largest
				largest = x
			} else {
				second = x
			}
			count = 2
			continue
		}
		if x > largest {
			second = largest
			largest = x
		} else if x < largest && x > second {
			second = x
		} else if x == second {
			continue
		}
	}
	if count < 2 {
		return 0, errors.New("fewer than two distinct values")
	}
	return second, nil
}
