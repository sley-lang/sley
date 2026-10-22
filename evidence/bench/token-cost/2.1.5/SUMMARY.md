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

## Program text vs other languages (correct program only, no tests)

### o200k_base

| task | python | sley_py | sley_fam_decl | c | rust | go | ts |
|---|---|---|---|---|---|---|---|
| fib | 35 | 35 | 39 | 36 | 40 | 37 | 40 |
| loopsum | 44 | 44 | 51 | 55 | 56 | 55 | 57 |
| collatz | 111 | 111 | 125 | 128 | 122 | 121 | 121 |
| primes | 84 | 84 | 96 | 104 | 100 | 95 | 100 |
| gcdsum | 83 | 83 | 116 | 109 | 101 | 99 | 109 |
| parse_int | 130 | 130 | 174 | 203 | 169 | 193 | 159 |
| transform | 44 | 44 | 51 | 110 | 65 | 66 | 68 |
| records | 54 | 54 | 45 | 52 | 58 | 66 | 49 |
| errors | 51 | 51 | 68 | 86 | 87 | 90 | 62 |
| **total** | 636 (1.00x) | 636 (1.00x) | 765 (1.20x) | 883 (1.39x) | 798 (1.25x) | 822 (1.29x) | 765 (1.20x) |

### cl100k_base

| task | python | sley_py | sley_fam_decl | c | rust | go | ts |
|---|---|---|---|---|---|---|---|
| fib | 35 | 35 | 39 | 36 | 40 | 37 | 40 |
| loopsum | 44 | 44 | 51 | 55 | 56 | 55 | 57 |
| collatz | 111 | 111 | 125 | 128 | 122 | 121 | 121 |
| primes | 84 | 84 | 96 | 104 | 100 | 95 | 100 |
| gcdsum | 83 | 83 | 116 | 109 | 101 | 100 | 109 |
| parse_int | 126 | 126 | 175 | 203 | 168 | 195 | 159 |
| transform | 44 | 44 | 51 | 110 | 65 | 66 | 68 |
| records | 55 | 55 | 45 | 52 | 58 | 66 | 49 |
| errors | 49 | 49 | 69 | 84 | 86 | 92 | 62 |
| **total** | 631 (1.00x) | 631 (1.00x) | 767 (1.22x) | 881 (1.40x) | 796 (1.26x) | 827 (1.31x) | 765 (1.21x) |

## One-time guide an agent reads to write each form
| form | guide | o200k | cl100k |
|---|---|---|---|
| sley_py | `help python` | 407 | 403 |
| sley_fam | `help card` | 311 | 311 |
| sley_af1 | `help (main) + help structured` | 1717 | 1717 |
| python | none (models already know Python) | 0 | 0 |

## Reading a program back (`sley-agent view <fn>` after the Python-dialect program, vs the Python source)
| task | Python source | AV1 view | view/source |
|---|---|---|---|
| fib | 35 | 356 | 10.2x |
| loopsum | 44 | 1324 | 30.1x |
| collatz | 111 | 4200 | 37.8x |
| primes | 84 | 2631 | 31.3x |
| gcdsum | 83 | 2709 | 32.6x |
| parse_int | 130 | 11963 | 92.0x |
| transform | 44 | 2215 | 50.3x |
| records | 54 | 570 | 10.6x |
| errors | 51 | 236 | 4.6x |

## Agent loop totals (o200k_base): write program+tests, run, read, fix, run, read
| scenario | form | tokens written | tokens read | total | vs Python | tool calls |
|---|---|---|---|---|---|---|
| clean | python | 1203 | 135 | 1338 | 1.00x | 9 |
| clean | sley_py | 1614 | 1179 | 2793 | 2.09x | 19 |
| clean | sley_py_inline | 1122 | 153 | 1275 | 0.95x | 9 |
| clean | sley_fam | 1825 | 1383 | 3208 | 2.40x | 21 |
| clean | sley_fam_decl | 1713 | 1093 | 2806 | 2.10x | 18 |
| clean | sley_af1 | 2354 | 153 | 2507 | 1.87x | 9 |
| clean | c | 1735 | 0 | 1735 | 1.30x | 9 |
| clean | rust | 1496 | 720 | 2216 | 1.66x | 9 |
| clean | go | 2120 | 108 | 2228 | 1.67x | 9 |
| clean | ts | 1603 | 9 | 1612 | 1.20x | 9 |
| logic | python | 1383 | 1307 | 2690 | 1.00x | 18 |
| logic | sley_py | 1826 | 2667 | 4493 | 1.67x | 28 |
| logic | sley_py_inline | 1298 | 699 | 1997 | 0.74x | 18 |
| logic | sley_fam | 2043 | 2890 | 4933 | 1.83x | 30 |
| logic | sley_fam_decl | 1931 | 2600 | 4531 | 1.68x | 27 |
| logic | sley_af1 | 2594 | 1660 | 4254 | 1.58x | 18 |
| logic | c | 2000 | 327 | 2327 | 0.87x | 18 |
| logic | rust | 1620 | 2026 | 3646 | 1.36x | 18 |
| logic | go | 2229 | 730 | 2959 | 1.10x | 18 |
| logic | ts | 1795 | 1872 | 3667 | 1.36x | 18 |
| typo | python | 1386 | 2547 | 3933 | 1.00x | 18 |
| typo | sley_py | 1797 | 1521 | 3318 | 0.84x | 28 |
| typo | sley_py_inline | 1301 | 495 | 1796 | 0.46x | 18 |
| typo | sley_fam | 2025 | 1730 | 3755 | 0.95x | 30 |
| typo | sley_fam_decl | 1901 | 1444 | 3345 | 0.85x | 27 |
| typo | sley_af1 | 2603 | 1436 | 4039 | 1.03x | 18 |
| typo | c | 1998 | 953 | 2951 | 0.75x | 18 |
| typo | rust | 1618 | 2136 | 3754 | 0.95x | 18 |
| typo | go | 2220 | 349 | 2569 | 0.65x | 18 |
| typo | ts | 1797 | 192 | 1989 | 0.51x | 18 |

### Per task, logic-bug loop (o200k_base, total tokens)
| task | python | sley_py | sley_py_inline | sley_fam | sley_fam_decl | sley_af1 | c | rust | go | ts |
|---|---|---|---|---|---|---|---|---|---|---|
| fib | 245 | 468 (1.91x) | 211 (0.86x) | 474 (1.93x) | 474 (1.93x) | 405 (1.65x) | 179 (0.73x) | 353 (1.44x) | 295 (1.20x) | 365 (1.49x) |
| loopsum | 261 | 448 (1.72x) | 200 (0.77x) | 457 (1.75x) | 457 (1.75x) | 391 (1.50x) | 200 (0.77x) | 366 (1.40x) | 274 (1.05x) | 379 (1.45x) |
| collatz | 297 | 503 (1.69x) | 249 (0.84x) | 519 (1.75x) | 519 (1.75x) | 505 (1.70x) | 250 (0.84x) | 414 (1.39x) | 308 (1.04x) | 428 (1.44x) |
| primes | 265 | 474 (1.79x) | 220 (0.83x) | 488 (1.84x) | 488 (1.84x) | 471 (1.78x) | 228 (0.86x) | 394 (1.49x) | 270 (1.02x) | 406 (1.53x) |
| gcdsum | 285 | 510 (1.79x) | 235 (0.82x) | 542 (1.90x) | 542 (1.90x) | 527 (1.85x) | 247 (0.87x) | 406 (1.42x) | 287 (1.01x) | 422 (1.48x) |
| parse_int | 377 | 580 (1.54x) | 321 (0.85x) | 781 (2.07x) | 646 (1.71x) | 694 (1.84x) | 455 (1.21x) | 530 (1.41x) | 588 (1.56x) | 520 (1.38x) |
| transform | 264 | 427 (1.62x) | 172 (0.65x) | 436 (1.65x) | 436 (1.65x) | 403 (1.53x) | 276 (1.05x) | 371 (1.41x) | 282 (1.07x) | 387 (1.47x) |
| records | 328 | 624 (1.90x) | 190 (0.58x) | 604 (1.84x) | 470 (1.43x) | 427 (1.30x) | 214 (0.65x) | 393 (1.20x) | 278 (0.85x) | 396 (1.21x) |
| errors | 368 | 459 (1.25x) | 199 (0.54x) | 632 (1.72x) | 499 (1.36x) | 431 (1.17x) | 278 (0.76x) | 419 (1.14x) | 377 (1.02x) | 364 (0.99x) |

## Agent loop totals (cl100k_base): write program+tests, run, read, fix, run, read
| scenario | form | tokens written | tokens read | total | vs Python | tool calls |
|---|---|---|---|---|---|---|
| clean | python | 1191 | 135 | 1326 | 1.00x | 9 |
| clean | sley_py | 1607 | 1167 | 2774 | 2.09x | 19 |
| clean | sley_py_inline | 1111 | 153 | 1264 | 0.95x | 9 |
| clean | sley_fam | 1823 | 1369 | 3192 | 2.41x | 21 |
| clean | sley_fam_decl | 1713 | 1082 | 2795 | 2.11x | 18 |
| clean | sley_af1 | 2348 | 153 | 2501 | 1.89x | 9 |
| clean | c | 1723 | 0 | 1723 | 1.30x | 9 |
| clean | rust | 1491 | 720 | 2211 | 1.67x | 9 |
| clean | go | 2114 | 108 | 2222 | 1.68x | 9 |
| clean | ts | 1596 | 9 | 1605 | 1.21x | 9 |
| logic | python | 1370 | 1282 | 2652 | 1.00x | 18 |
| logic | sley_py | 1819 | 2649 | 4468 | 1.68x | 28 |
| logic | sley_py_inline | 1287 | 695 | 1982 | 0.75x | 18 |
| logic | sley_fam | 2041 | 2870 | 4911 | 1.85x | 30 |
| logic | sley_fam_decl | 1931 | 2583 | 4514 | 1.70x | 27 |
| logic | sley_af1 | 2588 | 1654 | 4242 | 1.60x | 18 |
| logic | c | 1983 | 324 | 2307 | 0.87x | 18 |
| logic | rust | 1615 | 2013 | 3628 | 1.37x | 18 |
| logic | go | 2223 | 729 | 2952 | 1.11x | 18 |
| logic | ts | 1788 | 1828 | 3616 | 1.36x | 18 |
| typo | python | 1373 | 2564 | 3937 | 1.00x | 18 |
| typo | sley_py | 1790 | 1509 | 3299 | 0.84x | 28 |
| typo | sley_py_inline | 1290 | 495 | 1785 | 0.45x | 18 |
| typo | sley_fam | 2023 | 1716 | 3739 | 0.95x | 30 |
| typo | sley_fam_decl | 1901 | 1433 | 3334 | 0.85x | 27 |
| typo | sley_af1 | 2596 | 1428 | 4024 | 1.02x | 18 |
| typo | c | 1981 | 949 | 2930 | 0.74x | 18 |
| typo | rust | 1611 | 2127 | 3738 | 0.95x | 18 |
| typo | go | 2214 | 349 | 2563 | 0.65x | 18 |
| typo | ts | 1788 | 192 | 1980 | 0.50x | 18 |

### Per task, logic-bug loop (cl100k_base, total tokens)
| task | python | sley_py | sley_py_inline | sley_fam | sley_fam_decl | sley_af1 | c | rust | go | ts |
|---|---|---|---|---|---|---|---|---|---|---|
| fib | 243 | 467 (1.92x) | 211 (0.87x) | 473 (1.95x) | 473 (1.95x) | 405 (1.67x) | 179 (0.74x) | 352 (1.45x) | 297 (1.22x) | 360 (1.48x) |
| loopsum | 251 | 447 (1.78x) | 197 (0.78x) | 456 (1.82x) | 456 (1.82x) | 391 (1.56x) | 189 (0.75x) | 360 (1.43x) | 274 (1.09x) | 371 (1.48x) |
| collatz | 295 | 502 (1.70x) | 249 (0.84x) | 518 (1.76x) | 518 (1.76x) | 505 (1.71x) | 250 (0.85x) | 413 (1.40x) | 308 (1.04x) | 423 (1.43x) |
| primes | 263 | 473 (1.80x) | 220 (0.84x) | 487 (1.85x) | 487 (1.85x) | 471 (1.79x) | 228 (0.87x) | 393 (1.49x) | 270 (1.03x) | 401 (1.52x) |
| gcdsum | 283 | 499 (1.76x) | 233 (0.82x) | 531 (1.88x) | 531 (1.88x) | 516 (1.82x) | 245 (0.87x) | 403 (1.42x) | 286 (1.01x) | 415 (1.47x) |
| parse_int | 368 | 575 (1.56x) | 314 (0.85x) | 779 (2.12x) | 646 (1.76x) | 694 (1.89x) | 453 (1.23x) | 528 (1.43x) | 578 (1.57x) | 515 (1.40x) |
| transform | 262 | 426 (1.63x) | 172 (0.66x) | 435 (1.66x) | 435 (1.66x) | 402 (1.53x) | 276 (1.05x) | 370 (1.41x) | 282 (1.08x) | 382 (1.46x) |
| records | 327 | 623 (1.91x) | 191 (0.58x) | 602 (1.84x) | 469 (1.43x) | 427 (1.31x) | 212 (0.65x) | 392 (1.20x) | 278 (0.85x) | 389 (1.19x) |
| errors | 360 | 456 (1.27x) | 195 (0.54x) | 630 (1.75x) | 499 (1.39x) | 431 (1.20x) | 275 (0.76x) | 417 (1.16x) | 379 (1.05x) | 360 (1.00x) |
