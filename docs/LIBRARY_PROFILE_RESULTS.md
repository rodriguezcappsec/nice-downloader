# Library Profile Results

These measurements are directional evidence, not release thresholds. Repeat each scenario at least three times before using it to justify an optimization.

## 2026-08-13 local SSD baseline

- Branch: `feat/library-accessibility-performance` working tree
- Build: Rust debug test profile, single test thread
- Host: Linux 7.1.8 x86-64, Rust 1.97.1
- Filesystem: Btrfs on local NVMe SSD with async discard
- Synthetic items: 50,000
- SQLite main database: 35,262,464 bytes

| Operation | Time |
| --- | ---: |
| Open and migrate | 45 ms |
| Transactional seed | 653 ms |
| Count and first 60-item page | 9 ms |
| Case-insensitive title/path/tag search | 128 ms |
| Global counts, tags, and hierarchical folder facets | 197 ms |

The generated database was removed after the run.

## 2026-08-14 removable-drive baseline

- Branch: `feat/library-accessibility-performance` working tree
- Build: Rust debug test profile, single test thread
- Host: Linux 7.1.8 x86-64, Rust 1.97.1
- Filesystem: exFAT on an encrypted external USB hard drive
- Synthetic items: 50,000
- SQLite main database: 27,869,184 bytes

| Operation | Run 1 | Run 2 | Run 3 |
| --- | ---: | ---: | ---: |
| Open and migrate | 331 ms | 320 ms | 318 ms |
| Transactional seed | 1,119 ms | 1,164 ms | 1,097 ms |
| Count and fetch first 60-item page | 9 ms | 9 ms | 9 ms |
| Case-insensitive title/path/tag search | 115 ms | 112 ms | 114 ms |
| Global counts, tags, and hierarchical folder facets | 191 ms | 191 ms | 195 ms |

The repeated query timings remain close to the local baseline. Opening and seeding are slower on exFAT, but no user-facing query bottleneck was observed. The generated databases were removed after recording the runs.

Network-mount results remain outstanding; use the same item count, build profile, and host where possible.
