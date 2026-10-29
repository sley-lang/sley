# Where the money goes (micro-dollars per trial, list price)

| Language | uncached input | cache reads | cache writes | thinking | visible output | Total | Thinking tok / call | Visible output tok / call | Program tok / trial (o200k) |
|---|---|---|---|---|---|---|---|---|---|
| c | 0.2 | 7.6 | 186.8 | 135.7 | 151.6 | 482.0 | 267 | 298 | 197 |
| go | 0.2 | 7.0 | 180.6 | 114.8 | 128.0 | 430.5 | 225 | 251 | 170 |
| python | 0.2 | 6.9 | 169.5 | 40.9 | 74.0 | 291.5 | 82 | 148 | 102 |
| rust | 0.2 | 7.0 | 172.2 | 56.7 | 99.6 | 335.7 | 113 | 199 | 135 |
| sley | 0.2 | 14.0 | 181.4 | 96.6 | 59.1 | 351.3 | 186 | 114 | 83 |
| ts | 0.2 | 6.9 | 172.2 | 48.7 | 91.3 | 319.3 | 97 | 183 | 126 |
