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

The generated database was removed after the run. Removable-drive and network-mount results remain outstanding; use the same item count, build profile, and host where possible.
