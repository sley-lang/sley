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
| records | 54 | 95* | 73* | 137 | 1.76 | 1.35 | 2.54 |
| errors | 51 | 51 | 94* | 132 | 1.00 | 1.84 | 2.59 |
| **total** | 636 | 677 | 847 | 1651 | 1.06 | 1.33 | 2.60 |

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
| records | 55 | 96* | 73* | 137 | 1.75 | 1.33 | 2.49 |
| errors | 49 | 49 | 94* | 132 | 1.00 | 1.92 | 2.69 |
| **total** | 631 | 672 | 847 | 1647 | 1.06 | 1.34 | 2.61 |

\* includes the AF1 `types` frame the form needs (familiar syntax has no type declarations; the 2.1.3 Python dialect skips classes, so records need one there; the dialect lowers raise to a trap and needs none for errors).

## One-time guide an agent reads to write each form
| form | guide | o200k | cl100k |
|---|---|---|---|
| sley_py | `help python` | 252 | 250 |
| sley_fam | `help card` | 311 | 311 |
| sley_af1 | `help (main) + help structured` | 1717 | 1717 |
| python | none (models already know Python) | 0 | 0 |

## Reading a program back (`sley-agent view <fn>` after the Python-dialect program, vs the Python source)
| task | Python source | AV1 view | view/source |
|---|---|---|---|
| fib | 35 | 357 | 10.2x |
| loopsum | 44 | 1320 | 30.0x |
| collatz | 111 | 4201 | 37.8x |
| primes | 84 | 2635 | 31.4x |
| gcdsum | 83 | 2709 | 32.6x |
| parse_int | 130 | 11964 | 92.0x |
| transform | 44 | 2212 | 50.3x |
| records | 54 | 567 | 10.5x |
| errors | 51 | 234 | 4.6x |

## Agent loop totals (o200k_base): write program+tests, run, read, fix, run, read
| scenario | form | tokens written | tokens read | total | vs Python | tool calls |
|---|---|---|---|---|---|---|
| clean | python | 1203 | 135 | 1338 | 1.00x | 9 |
| clean | sley_py | 1614 | 2703 | 4317 | 3.23x | 19 |
| clean | sley_fam | 1825 | 2911 | 4736 | 3.54x | 21 |
| clean | sley_af1 | 2354 | 1568 | 3922 | 2.93x | 9 |
| logic | python | 1383 | 1307 | 2690 | 1.00x | 18 |
| logic | sley_py | 1826 | 4921 | 6747 | 2.51x | 28 |
| logic | sley_fam | 2043 | 5145 | 7188 | 2.67x | 30 |
| logic | sley_af1 | 2594 | 3785 | 6379 | 2.37x | 18 |
| typo | python | 1386 | 2547 | 3933 | 1.00x | 18 |
| typo | sley_py | 1797 | 4442 | 6239 | 1.59x | 28 |
| typo | sley_fam | 2025 | 4638 | 6663 | 1.69x | 30 |
| typo | sley_af1 | 2603 | 2851 | 5454 | 1.39x | 18 |

### Per task, logic-bug loop (o200k_base, total tokens)
| task | python | sley_py | sley_fam | sley_af1 |
|---|---|---|---|---|
| fib | 245 | 694 (2.83x) | 700 (2.86x) | 631 (2.58x) |
| loopsum | 261 | 703 (2.69x) | 712 (2.73x) | 633 (2.43x) |
| collatz | 297 | 745 (2.51x) | 761 (2.56x) | 733 (2.47x) |
| primes | 265 | 722 (2.72x) | 736 (2.78x) | 703 (2.65x) |
| gcdsum | 285 | 722 (2.53x) | 754 (2.65x) | 739 (2.59x) |
| parse_int | 377 | 972 (2.58x) | 1169 (3.10x) | 1015 (2.69x) |
| transform | 264 | 680 (2.58x) | 689 (2.61x) | 636 (2.41x) |
| records | 328 | 828 (2.52x) | 808 (2.46x) | 631 (1.92x) |
| errors | 368 | 681 (1.85x) | 859 (2.33x) | 658 (1.79x) |

## Agent loop totals (cl100k_base): write program+tests, run, read, fix, run, read
| scenario | form | tokens written | tokens read | total | vs Python | tool calls |
|---|---|---|---|---|---|---|
| clean | python | 1191 | 135 | 1326 | 1.00x | 9 |
| clean | sley_py | 1607 | 2687 | 4294 | 3.24x | 19 |
| clean | sley_fam | 1823 | 2893 | 4716 | 3.56x | 21 |
| clean | sley_af1 | 2348 | 1564 | 3912 | 2.95x | 9 |
| logic | python | 1370 | 1282 | 2652 | 1.00x | 18 |
| logic | sley_py | 1819 | 4899 | 6718 | 2.53x | 28 |
| logic | sley_fam | 2041 | 5121 | 7162 | 2.70x | 30 |
| logic | sley_af1 | 2588 | 3775 | 6363 | 2.40x | 18 |
| typo | python | 1373 | 2564 | 3937 | 1.00x | 18 |
| typo | sley_py | 1790 | 4427 | 6217 | 1.58x | 28 |
| typo | sley_fam | 2023 | 4621 | 6644 | 1.69x | 30 |
| typo | sley_af1 | 2596 | 2839 | 5435 | 1.38x | 18 |

### Per task, logic-bug loop (cl100k_base, total tokens)
| task | python | sley_py | sley_fam | sley_af1 |
|---|---|---|---|---|
| fib | 243 | 693 (2.85x) | 699 (2.88x) | 631 (2.60x) |
| loopsum | 251 | 702 (2.80x) | 711 (2.83x) | 633 (2.52x) |
| collatz | 295 | 744 (2.52x) | 760 (2.58x) | 733 (2.48x) |
| primes | 263 | 721 (2.74x) | 735 (2.79x) | 703 (2.67x) |
| gcdsum | 283 | 707 (2.50x) | 739 (2.61x) | 724 (2.56x) |
| parse_int | 368 | 967 (2.63x) | 1167 (3.17x) | 1015 (2.76x) |
| transform | 262 | 679 (2.59x) | 688 (2.63x) | 635 (2.42x) |
| records | 327 | 827 (2.53x) | 806 (2.46x) | 631 (1.93x) |
| errors | 360 | 678 (1.88x) | 857 (2.38x) | 658 (1.83x) |
