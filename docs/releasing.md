# Releasing

Releases are made only when the maintainer calls one. Nothing is released
automatically from `main`.

## Version numbers

* Releases go 0.1.0 → 0.2.0 → 0.3.0 … and are referred to as "0.1", "0.2", …
* A point release (0.2.1) ships finished work, or an urgent fix, between
  minor releases, when the maintainer calls one. The steps are the same.
* The version lives in one place: `[workspace.package] version` in the root
  `Cargo.toml`. Every crate and app inherits it, and the About window and
  `--version` read it at build time.
* 1.0 is reserved for when LedgerCraft is ready to keep real books (see ROADMAP.md).

## Steps for a release (example: 0.2)

1. Make sure `main` is green: `cargo xtask ci`.
2. Set the version in `Cargo.toml`: `version = "0.2.0"`, then run
   `cargo check` so `Cargo.lock` updates.
3. In `CHANGELOG.md`, rename `[Unreleased]` to `[0.2.0] - YYYY-MM-DD` and add
   a fresh empty `[Unreleased]` section above it. Update the stage line in
   `ROADMAP.md`.
4. Commit: `git commit -am "Release prep: 0.2.0"` and push.
5. Tag and push the tag — this is the step that publishes:
   ```
   git tag -a v0.2.0 -m "LedgerCraft 0.2.0"
   git push origin v0.2.0
   ```
6. The Release workflow (`.github/workflows/release.yml`) checks that the tag
   matches the version in `Cargo.toml`, builds the Windows (`.zip`) and Linux
   (`.tar.gz`, with `install.sh`, a desktop entry and the icon) packages, and
   creates a **draft** GitHub Release with them and `SHA256SUMS.txt`
   attached. macOS joins once it has been tested.
7. Review the draft on GitHub (Releases), paste the changelog section into the
   notes, and press **Publish release**.

## Not yet done

* Code signing (Windows Authenticode, macOS notarization). Until then, Windows
  SmartScreen and macOS Gatekeeper will warn that the app is unsigned.
* Installers (MSI, DMG, AppImage, AUR PKGBUILD, deb/rpm). Releases ship a
  zip for Windows and a tarball with a per-user `install.sh` for Linux.
