## Program text (correct program only, no tests)

### o200k_base

| task | Python | Sley Python dialect | Sley familiar | Sley AF1-X JSON | dialect/Py | familiar/Py | AF1/Py |
|---|---|---|---|---|---|---|---|
| fib | 35 | 35 | 39 | 102 | 1.00 | 1.11 | 2.91 |
| loopsum | 44 | 44 | 51 | 110 | 1.00 | 1.16 | 2.50 |
| collatz | 111 | 111 | 125 | 236 | 1.00 | 1.13 | 2.13 |
| primes | 84 | 84 | 96 | 196 | 1.00 | 1.14 | 2.33 |
| gcdsum | 83 | 83 | 116 | 238 | 1.00 | 1.40 | 2.87 |
| parse_int | 130 | 130 | 202* | 356 | 1.00 | 1.55 | 2.74 |
| transform | 44 | 44 | 51 | 144 | 1.00 | 1.16 | 3.27 |
| records | 54 | 54 | 73* | 137 | 1.00 | 1.35 | 2.54 |
| errors | 51 | 51 | 94* | 132 | 1.00 | 1.84 | 2.59 |
| **total** | 636 | 636 | 847 | 1651 | 1.00 | 1.33 | 2.60 |

### cl100k_base

| task | Python | Sley Python dialect | Sley familiar | Sley AF1-X JSON | dialect/Py | familiar/Py | AF1/Py |
|---|---|---|---|---|---|---|---|
| fib | 35 | 35 | 39 | 102 | 1.00 | 1.11 | 2.91 |
| loopsum | 44 | 44 | 51 | 110 | 1.00 | 1.16 | 2.50 |
| collatz | 111 | 111 | 125 | 236 | 1.00 | 1.13 | 2.13 |
| primes | 84 | 84 | 96 | 196 | 1.00 | 1.14 | 2.33 |
| gcdsum | 83 | 83 | 116 | 235 | 1.00 | 1.40 | 2.83 |
| parse_int | 126 | 126 | 202* | 356 | 1.00 | 1.60 | 2.83 |
| transform | 44 | 44 | 51 | 143 | 1.00 | 1.16 | 3.25 |
| records | 55 | 55 | 73* | 137 | 1.00 | 1.33 | 2.49 |
| errors | 49 | 49 | 94* | 132 | 1.00 | 1.92 | 2.69 |
| **total** | 631 | 631 | 847 | 1647 | 1.00 | 1.34 | 2.61 |

\* includes the AF1 `types` frame the form needs (familiar syntax has no type declarations; the 2.1.3 Python dialect skips classes, so records need one there; the dialect lowers raise to a trap and needs none for errors).

## One-time guide an agent reads to write each form
| form | guide | o200k | cl100k |
|---|---|---|---|
| sley_py | `help python` | 322 | 319 |
| sley_fam | `help card` | 311 | 311 |
| sley_af1 | `help (main) + help structured` | 1717 | 1717 |
| python | none (models already know Python) | 0 | 0 |

## Reading a program back (`sley-agent view <fn>` after the Python-dialect program, vs the Python source)
| task | Python source | AV1 view | view/source |
|---|---|---|---|
| fib | 35 | 356 | 10.2x |
| loopsum | 44 | 1320 | 30.0x |
| collatz | 111 | 4201 | 37.8x |
| primes | 84 | 2630 | 31.3x |
| gcdsum | 83 | 2712 | 32.7x |
| parse_int | 130 | 11965 | 92.0x |
| transform | 44 | 2213 | 50.3x |
| records | 54 | 566 | 10.5x |
| errors | 51 | 236 | 4.6x |

## Agent loop totals (o200k_base): write program+tests, run, read, fix, run, read
| scenario | form | tokens written | tokens read | total | vs Python | tool calls |
|---|---|---|---|---|---|---|
| clean | python | 1203 | 135 | 1338 | 1.00x | 9 |
| clean | sley_py | 1614 | 1377 | 2991 | 2.24x | 19 |
| clean | sley_py_inline | 1122 | 351 | 1473 | 1.10x | 9 |
| clean | sley_fam | 1825 | 1581 | 3406 | 2.55x | 21 |
| clean | sley_af1 | 2354 | 351 | 2705 | 2.02x | 9 |
| logic | python | 1383 | 1307 | 2690 | 1.00x | 18 |
| logic | sley_py | 1826 | 2865 | 4691 | 1.74x | 28 |
| logic | sley_py_inline | 1298 | 1618 | 2916 | 1.08x | 18 |
| logic | sley_fam | 2043 | 3088 | 5131 | 1.91x | 30 |
| logic | sley_af1 | 2594 | 1858 | 4452 | 1.66x | 18 |
| typo | python | 1386 | 2548 | 3934 | 1.00x | 18 |
| typo | sley_py | 1797 | 2598 | 4395 | 1.12x | 28 |
| typo | sley_py_inline | 1301 | 1572 | 2873 | 0.73x | 18 |
| typo | sley_fam | 2025 | 2794 | 4819 | 1.22x | 30 |
| typo | sley_af1 | 2603 | 1634 | 4237 | 1.08x | 18 |

### Per task, logic-bug loop (o200k_base, total tokens)
| task | python | sley_py | sley_py_inline | sley_fam | sley_af1 |
|---|---|---|---|---|---|
| fib | 245 | 490 (2.00x) | 315 (1.29x) | 496 (2.02x) | 427 (1.74x) |
| loopsum | 261 | 470 (1.80x) | 299 (1.15x) | 479 (1.84x) | 413 (1.58x) |
| collatz | 297 | 525 (1.77x) | 351 (1.18x) | 541 (1.82x) | 527 (1.77x) |
| primes | 265 | 496 (1.87x) | 322 (1.22x) | 510 (1.92x) | 493 (1.86x) |
| gcdsum | 285 | 532 (1.87x) | 349 (1.22x) | 564 (1.98x) | 549 (1.93x) |
| parse_int | 377 | 602 (1.60x) | 419 (1.11x) | 803 (2.13x) | 716 (1.90x) |
| transform | 264 | 449 (1.70x) | 270 (1.02x) | 458 (1.73x) | 425 (1.61x) |
| records | 328 | 646 (1.97x) | 295 (0.90x) | 626 (1.91x) | 449 (1.37x) |
| errors | 368 | 481 (1.31x) | 296 (0.80x) | 654 (1.78x) | 453 (1.23x) |

## Agent loop totals (cl100k_base): write program+tests, run, read, fix, run, read
| scenario | form | tokens written | tokens read | total | vs Python | tool calls |
|---|---|---|---|---|---|---|
| clean | python | 1191 | 135 | 1326 | 1.00x | 9 |
| clean | sley_py | 1607 | 1365 | 2972 | 2.24x | 19 |
| clean | sley_py_inline | 1111 | 351 | 1462 | 1.10x | 9 |
| clean | sley_fam | 1823 | 1567 | 3390 | 2.56x | 21 |
| clean | sley_af1 | 2348 | 351 | 2699 | 2.04x | 9 |
| logic | python | 1370 | 1282 | 2652 | 1.00x | 18 |
| logic | sley_py | 1819 | 2847 | 4666 | 1.76x | 28 |
| logic | sley_py_inline | 1287 | 1611 | 2898 | 1.09x | 18 |
| logic | sley_fam | 2041 | 3068 | 5109 | 1.93x | 30 |
| logic | sley_af1 | 2588 | 1852 | 4440 | 1.67x | 18 |
| typo | python | 1373 | 2565 | 3938 | 1.00x | 18 |
| typo | sley_py | 1790 | 2587 | 4377 | 1.11x | 28 |
| typo | sley_py_inline | 1290 | 1573 | 2863 | 0.73x | 18 |
| typo | sley_fam | 2023 | 2781 | 4804 | 1.22x | 30 |
| typo | sley_af1 | 2596 | 1626 | 4222 | 1.07x | 18 |

### Per task, logic-bug loop (cl100k_base, total tokens)
| task | python | sley_py | sley_py_inline | sley_fam | sley_af1 |
|---|---|---|---|---|---|
| fib | 243 | 489 (2.01x) | 315 (1.30x) | 495 (2.04x) | 427 (1.76x) |
| loopsum | 251 | 469 (1.87x) | 295 (1.18x) | 478 (1.90x) | 413 (1.65x) |
| collatz | 295 | 524 (1.78x) | 351 (1.19x) | 540 (1.83x) | 527 (1.79x) |
| primes | 263 | 495 (1.88x) | 322 (1.22x) | 509 (1.94x) | 493 (1.87x) |
| gcdsum | 283 | 521 (1.84x) | 345 (1.22x) | 553 (1.95x) | 538 (1.90x) |
| parse_int | 368 | 597 (1.62x) | 412 (1.12x) | 801 (2.18x) | 716 (1.95x) |
| transform | 262 | 448 (1.71x) | 270 (1.03x) | 457 (1.74x) | 424 (1.62x) |
| records | 327 | 645 (1.97x) | 296 (0.91x) | 624 (1.91x) | 449 (1.37x) |
| errors | 360 | 478 (1.33x) | 292 (0.81x) | 652 (1.81x) | 453 (1.26x) |
