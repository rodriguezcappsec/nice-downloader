# Bundled engine staging directory

Run `npm run engines:prepare` before a bundled release build. Generated binaries
are intentionally ignored by Git; Tauri packages this directory as application
resources. Versions and upstream SHA-256 digests are pinned in
`engines.lock.json`.
