# Release and Distribution

Tagged `v*` revisions build Linux AppImage/DEB and Windows NSIS artifacts in separate GitHub Actions workflows. Run CI first; artifacts are intentionally unsigned until project-owned credentials are configured.

## Local release checks

```bash
npm ci
npm run typecheck
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --all-targets --no-fail-fast
npm run tauri:build -- --bundles appimage,deb
```

Test AppImages on Ubuntu and Arch-based hosts. The AppImage requires a compatible FUSE setup or extraction via `--appimage-extract-and-run`. The Windows workflow produces an NSIS `setup.exe`; test installation, WebView2 bootstrap, PATH engine discovery, cancellation, and Unicode paths on a real Windows runner.

## Flatpak foundation

`packaging/io.github.nicedownloader.desktop.yml` is a development manifest. A production Flatpak must vendor all Cargo/npm sources, install desktop metadata/icons, and define a portal-aware engine/credential strategy before submission to Flathub.

## External release blockers

- Configure Tauri updater/signing keys in repository secrets and publish public-key metadata.
- Obtain a Windows Authenticode certificate and sign both executable and NSIS installer.
- Decide Linux artifact signing and checksum publication policy.
- Execute release smoke tests on clean Arch, Ubuntu, and Windows machines.
- Produce an SBOM and archive exact engine versions/checksums for each release.

Never commit private keys, cookie files, proxy credentials, or signing passwords.
