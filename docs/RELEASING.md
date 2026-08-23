# Release and Distribution

Tagged `v*` revisions build Linux AppImage and DEB artifacts. Nice Downloader
does not publish or support Windows or macOS builds. Run CI first; artifacts
are intentionally unsigned until project-owned credentials are configured.

## Local release checks

```bash
npm ci
npm run typecheck
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --all-targets --no-fail-fast
npm run tauri:build:bundled -- --bundles deb
npm run tauri:build:appimage:arch
npm run tauri:build:appimage:arch:slim
npm run tauri:build:slim -- --bundles deb
```

`tauri:build` aliases the standard bundled profile. Tagged releases currently publish that profile. The Slim command is an explicit distribution option and must remain engine-resource-free; inspect its package contents before publishing a Slim artifact. See `docs/BUNDLED_ENGINES.md` for profile behavior.

On Arch Linux, an unrelated unreadable symlink below `/usr/bin` can crash
`linuxdeploy`, and the bundled `strip` may not understand modern `.relr.dyn`
sections. Install Bubblewrap, then use:

```bash
sudo pacman -S bubblewrap
npm run tauri:build:appimage:arch
```

The script sets `NO_STRIP=1` and runs `linuxdeploy` directly on ordinary build
hosts. When it detects an unreadable `/var/lib/searxng`, it uses Bubblewrap to
mask only that path inside the build process's mount namespace. It does not
change system permissions.
The AppImage wrapper stages a curated GStreamer playback plugin set and matching
scanner, reruns `linuxdeploy` so their shared-library dependencies are included,
excludes the build host's `libwayland-client` so WebKitGTK uses the target host's
graphics boundary,
and installs the project AppRun launcher before generating the final image. The
output is written to `src-tauri/target/release/bundle/appimage/`.

Test AppImages on Ubuntu and Arch-based hosts. AppImage requires compatible
FUSE setup or extraction via `--appimage-extract-and-run`.

## Flatpak foundation

`packaging/io.github.nicedownloader.desktop.yml` is a development manifest. A
production Flatpak must vendor all Cargo/npm sources, install desktop
metadata/icons, and define a portal-aware engine and credential strategy before
submission to Flathub.

## External release blockers

- Configure Tauri updater/signing keys in repository secrets and publish
  public-key metadata.
- Decide the Linux artifact signing and checksum publication policy.
- Execute release smoke tests on clean Arch and Ubuntu machines.
- Produce an SBOM and archive exact engine versions/checksums per release.

Never commit private keys, cookie files, proxy credentials, or signing
passwords.
