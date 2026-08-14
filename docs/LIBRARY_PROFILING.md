# Library Performance Profiling

The ignored Rust harness creates a synthetic catalog and reports timings for database open, transactional seed, first page, text search, and facets. It does not create or inspect media files.

Run the default 50,000-item profile on temporary local storage:

```bash
cargo test --manifest-path src-tauri/Cargo.toml \
  database::tests::profiles_large_library_queries_on_a_selected_filesystem \
  -- --ignored --nocapture --test-threads=1
```

To exercise a removable drive or mounted network filesystem, select an existing directory:

```bash
NICE_DOWNLOADER_LIBRARY_PROFILE_DIR=/mnt/media-profile \
NICE_DOWNLOADER_LIBRARY_PROFILE_ITEMS=50000 \
cargo test --manifest-path src-tauri/Cargo.toml \
  database::tests::profiles_large_library_queries_on_a_selected_filesystem \
  -- --ignored --nocapture --test-threads=1
```

The item count is bounded from 10,000 through 250,000. External runs retain a uniquely named `nice-downloader-profile-*.sqlite3` database so its size can be inspected; remove that exact file after recording results. Temporary runs clean themselves up.

Compare results only on the same build profile and machine. Record filesystem/mount type, item count, database bytes, and all reported timings. Treat results as evidence for optimization rather than enforcing fragile wall-clock thresholds in CI.

Recorded baselines and their exact environment live in `docs/LIBRARY_PROFILE_RESULTS.md`.
