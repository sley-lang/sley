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
| sley_py | `help python` | 299 | 297 |
| sley_fam | `help card` | 311 | 311 |
| sley_af1 | `help (main) + help structured` | 1717 | 1717 |
| python | none (models already know Python) | 0 | 0 |

## Reading a program back (`sley-agent view <fn>` after the Python-dialect program, vs the Python source)
| task | Python source | AV1 view | view/source |
|---|---|---|---|
| fib | 35 | 359 | 10.3x |
| loopsum | 44 | 1324 | 30.1x |
| collatz | 111 | 4201 | 37.8x |
| primes | 84 | 2632 | 31.3x |
| gcdsum | 83 | 2710 | 32.7x |
| parse_int | 130 | 11962 | 92.0x |
| transform | 44 | 2211 | 50.2x |
| records | 54 | 568 | 10.5x |
| errors | 51 | 234 | 4.6x |

## Agent loop totals (o200k_base): write program+tests, run, read, fix, run, read
| scenario | form | tokens written | tokens read | total | vs Python | tool calls |
|---|---|---|---|---|---|---|
| clean | python | 1203 | 135 | 1338 | 1.00x | 9 |
| clean | sley_py | 1614 | 1882 | 3496 | 2.61x | 19 |
| clean | sley_py_inline | 1122 | 858 | 1980 | 1.48x | 9 |
| clean | sley_fam | 1825 | 2096 | 3921 | 2.93x | 21 |
| clean | sley_af1 | 2354 | 866 | 3220 | 2.41x | 9 |
| logic | python | 1383 | 1307 | 2690 | 1.00x | 18 |
| logic | sley_py | 1826 | 3370 | 5196 | 1.93x | 28 |
| logic | sley_py_inline | 1298 | 2060 | 3358 | 1.25x | 18 |
| logic | sley_fam | 2043 | 3603 | 5646 | 2.10x | 30 |
| logic | sley_af1 | 2594 | 2373 | 4967 | 1.85x | 18 |
| typo | python | 1386 | 2549 | 3935 | 1.00x | 18 |
| typo | sley_py | 1797 | 3103 | 4900 | 1.25x | 28 |
| typo | sley_py_inline | 1301 | 2079 | 3380 | 0.86x | 18 |
| typo | sley_fam | 2025 | 3309 | 5334 | 1.36x | 30 |
| typo | sley_af1 | 2603 | 2149 | 4752 | 1.21x | 18 |

### Per task, logic-bug loop (o200k_base, total tokens)
| task | python | sley_py | sley_py_inline | sley_fam | sley_af1 |
|---|---|---|---|---|---|
| fib | 245 | 546 (2.23x) | 360 (1.47x) | 552 (2.25x) | 483 (1.97x) |
| loopsum | 261 | 522 (2.00x) | 344 (1.32x) | 531 (2.03x) | 465 (1.78x) |
| collatz | 297 | 581 (1.96x) | 399 (1.34x) | 597 (2.01x) | 583 (1.96x) |
| primes | 265 | 552 (2.08x) | 370 (1.40x) | 566 (2.14x) | 549 (2.07x) |
| gcdsum | 285 | 600 (2.11x) | 409 (1.44x) | 632 (2.22x) | 617 (2.16x) |
| parse_int | 377 | 654 (1.73x) | 463 (1.23x) | 860 (2.28x) | 773 (2.05x) |
| transform | 264 | 502 (1.90x) | 319 (1.21x) | 511 (1.94x) | 478 (1.81x) |
| records | 328 | 706 (2.15x) | 351 (1.07x) | 686 (2.09x) | 509 (1.55x) |
| errors | 368 | 533 (1.45x) | 343 (0.93x) | 711 (1.93x) | 510 (1.39x) |

## Agent loop totals (cl100k_base): write program+tests, run, read, fix, run, read
| scenario | form | tokens written | tokens read | total | vs Python | tool calls |
|---|---|---|---|---|---|---|
| clean | python | 1191 | 135 | 1326 | 1.00x | 9 |
| clean | sley_py | 1607 | 1866 | 3473 | 2.62x | 19 |
| clean | sley_py_inline | 1111 | 855 | 1966 | 1.48x | 9 |
| clean | sley_fam | 1823 | 2078 | 3901 | 2.94x | 21 |
| clean | sley_af1 | 2348 | 862 | 3210 | 2.42x | 9 |
| logic | python | 1370 | 1282 | 2652 | 1.00x | 18 |
| logic | sley_py | 1819 | 3348 | 5167 | 1.95x | 28 |
| logic | sley_py_inline | 1287 | 2050 | 3337 | 1.26x | 18 |
| logic | sley_fam | 2041 | 3579 | 5620 | 2.12x | 30 |
| logic | sley_af1 | 2588 | 2363 | 4951 | 1.87x | 18 |
| typo | python | 1373 | 2566 | 3939 | 1.00x | 18 |
| typo | sley_py | 1790 | 3088 | 4878 | 1.24x | 28 |
| typo | sley_py_inline | 1290 | 2077 | 3367 | 0.85x | 18 |
| typo | sley_fam | 2023 | 3292 | 5315 | 1.35x | 30 |
| typo | sley_af1 | 2596 | 2137 | 4733 | 1.20x | 18 |

### Per task, logic-bug loop (cl100k_base, total tokens)
| task | python | sley_py | sley_py_inline | sley_fam | sley_af1 |
|---|---|---|---|---|---|
| fib | 243 | 545 (2.24x) | 360 (1.48x) | 551 (2.27x) | 483 (1.99x) |
| loopsum | 251 | 521 (2.08x) | 339 (1.35x) | 530 (2.11x) | 465 (1.85x) |
| collatz | 295 | 580 (1.97x) | 399 (1.35x) | 596 (2.02x) | 583 (1.98x) |
| primes | 263 | 551 (2.10x) | 370 (1.41x) | 565 (2.15x) | 549 (2.09x) |
| gcdsum | 283 | 585 (2.07x) | 403 (1.42x) | 617 (2.18x) | 602 (2.13x) |
| parse_int | 368 | 649 (1.76x) | 456 (1.24x) | 858 (2.33x) | 773 (2.10x) |
| transform | 262 | 501 (1.91x) | 319 (1.22x) | 510 (1.95x) | 477 (1.82x) |
| records | 327 | 705 (2.16x) | 352 (1.08x) | 684 (2.09x) | 509 (1.56x) |
| errors | 360 | 530 (1.47x) | 339 (0.94x) | 709 (1.97x) | 510 (1.42x) |
