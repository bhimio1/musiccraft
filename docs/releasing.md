# Releasing MusicCraft

Releases are made by one GitHub Action. You type a version number; it does the rest.

## Cut a release

1. On GitHub open **Actions → Release → Run workflow** (this works from a phone
   browser too).
2. Leave the branch on `main`, type the version (for example `0.2.0`) and press
   **Run workflow**.

That is all. The workflow:

1. checks the version is well formed (`1.2.3` or `1.2.3-beta.1`) and that `v<version>`
   has not been released before;
2. builds every package with that version applied (`packaging/set-version.sh` bumps
   `Cargo.toml` and `Cargo.lock` inside each build, so the binaries and installers carry
   the right number);
3. installs, upgrades and uninstalls the `.msi`, and mounts the `.dmg` to check it;
4. only if all of that passed, commits `Release v<version>` (the version bump) to the
   branch, tags that commit `v<version>`, and publishes the GitHub release with
   `SHA256SUMS.txt`.

If any build or test fails, nothing is committed, tagged or published; fix the problem
and run it again with the same version. A suffix (`0.2.0-beta.1`) publishes a
pre-release. The MSI always uses the numeric part (`0.2.0`), because MSI versions cannot
carry suffixes.

Untick **publish** for a dry run: everything is built and tested and the packages are
attached to the run, but nothing is committed or released.

The bump commit is pushed by the Actions bot. If `main` is protected so that only pull
requests may change it, that push is refused and the release stops before tagging; allow
the bot to bypass the rule, or bump the version yourself in a pull request
(`bash packaging/set-version.sh 0.2.0`) and run the workflow with that same version
(it then has nothing to commit and just tags).

## What gets published

| File | Contents |
| --- | --- |
| `MusicCraft-<v>-windows-x64.msi` | Installer: per-machine, `C:\Program Files\MusicCraft`, Start menu shortcut, in-place upgrades. |
| `MusicCraft-<v>-windows-x64-portable.zip` | `musiccraft.exe` plus a readme; no install. |
| `MusicCraft-<v>-macos-universal.dmg` | `MusicCraft.app` (Apple Silicon and Intel, macOS 11+) with an Applications shortcut. |
| `MusicCraft-<v>-linux-x86_64.flatpak` | Flatpak bundle, app ID `io.github.bhimio1.MusicCraft`. |
| `MusicCraft-<v>-linux-x86_64-portable.tar.gz` | Binary plus readme and icon; built on Ubuntu 22.04 (glibc 2.35+). |
| `MusicCraft-Plugins-<v>-linux-x86_64.zip`, `-windows-x64.zip`, `-macos-universal.zip` | MusicCraft Effects and MusicCraft Instruments as `.clap` and `.vst3` bundles, a `README.txt` with each system's plugin folders (`packaging/portable/README-plugins.txt`) and the license. macOS bundles are universal and ad-hoc signed. Each zip's bundles are checked before release: present, the right binary inside, and on Linux the CLAP entry point exported. The validators run on every CI build (`docs/spec/validation.md`). |
| `SHA256SUMS.txt` | Checksums for all of the above. |

Install the Flatpak with `flatpak install --user ./MusicCraft-<v>-linux-x86_64.flatpak`
(the freedesktop 25.08 runtime is pulled from Flathub).

## Testing packaging without releasing

The build and test jobs also run on pull requests that touch `packaging/**` or the
workflow (using the version already in `Cargo.toml`), and as a dry run from the **Run
workflow** button. Neither commits, tags or publishes anything: the packages are attached
to the run as artifacts.

## Known limitations

- **Plugins are not notarized either.** On macOS, clear the quarantine flag on the
  copied bundles (`xattr -dr com.apple.quarantine …`, see the zip's README) or the DAW
  may refuse to load them.
- **Unsigned builds.** Windows SmartScreen and macOS Gatekeeper warn on first launch.
  - macOS: right-click the app → Open, or `xattr -dr com.apple.quarantine /Applications/MusicCraft.app`.
  - To sign: `packaging/macos/build-dmg.sh` already honors `APPLE_SIGN_IDENTITY`, but the
    workflow does not yet import a certificate into the keychain or notarize. When
    signing, `packaging/macos/entitlements.plist` is applied so the hardened runtime
    still allows loading third-party CLAP plugins and recording audio. Windows
    signing (for example with `signtool` or Azure Trusted Signing) is not wired in.
- **License.** MusicCraft is GPL-3.0-or-later (`LICENSE`). Under the GPL, every
  binary release must come with its source (or a written offer of it): the GitHub
  release's tag provides it; keep releases tagged.
- **Flatpak is not Flathub-ready.** It packages the binary CI built. Flathub requires a
  build from source: switch the manifest to the `org.freedesktop.Sdk.Extension.rust-stable`
  extension with vendored crates (`cargo vendor`), and add screenshots to the metainfo.
- **Plugins in the Flatpak.** CLAP plugins are found in `~/.clap` and `CLAP_PATH`
  folders under the home directory, but the sandbox cannot see system-wide folders such
  as `/usr/lib/clap`, and plugins that depend on host libraries may not load. Use the
  portable tarball if you need system-installed plugins.
- **Flatpak permissions.** `--filesystem=home` is needed because the app's own file
  dialogs and sample browser walk the home folder; `--device=all` is for MIDI keyboards.
  Both can be narrowed if the app later uses desktop portals.
- **Only x86-64 on Linux and Windows.** Add an `aarch64` job if you need ARM builds.
- **App icon** is the owner's artwork in `packaging/icons/musiccraft-source.png`.
  `packaging/icons/make_icons.py` derives every size the packages read (PNGs at 128,
  256, 512 and 1024 px plus `musiccraft.ico`); replace the source and rerun it to
  change the icon. The app embeds the 256 px PNG as its window icon. The 1024 px PNG
  (macOS `.icns`) is upscaled from the 512 px source, so a larger source would sharpen
  it on Retina. The Windows exe itself has no embedded icon yet (the installer
  shortcut and the running window do).

## Local builds

- **Linux portable:** `cargo build --release -p musiccraft-app`, then copy
  `target/release/musiccraft`.
- **Flatpak:** copy `target/release/musiccraft` and `packaging/icons/musiccraft-{128,256,512}.png`
  into a scratch folder alongside the manifest, `.desktop` file and a `metainfo.xml`
  made from the `.in` template (replace `@VERSION@` and `@DATE@`), then run
  `flatpak-builder --user --install-deps-from=flathub --force-clean build-dir io.github.bhimio1.MusicCraft.yml`.
- **macOS:** `bash packaging/macos/build-dmg.sh <version> <binary> packaging/icons/musiccraft-1024.png <out.dmg>`.
- **Windows:** install WiX v4 (`dotnet tool install --global wix --version 4.0.5`) and see
  the command at the top of `packaging/windows/musiccraft.wxs`.
