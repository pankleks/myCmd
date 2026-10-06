# CI, testing and releases

## CI

The **CI** workflow (`.github/workflows/check.yml`) runs on branch pushes, pull requests and manual dispatch. The release workflow also calls it for the exact commit referenced by the release tag.

| Stage                        | Checks                                                                                                                         |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| Frontend and release tooling | actionlint, Prettier, version consistency (optional changelog), Vitest with coverage thresholds, TypeScript/Svelte, Vite build |
| Rust and desktop             | Windows / Ubuntu 22.04 / macOS: rustfmt, Clippy without warnings, Rust tests                                                   |
| Windows desktop              | Debug build with the frontend embedded; a real Tauri and WebView2 process test                                                 |
| CI passed                    | Combined required status; succeeds only when all previous stages succeed                                                       |

Installation uses `npm ci`, and Cargo uses `--locked`. Both lockfiles must be committed. Rust and npm dependency caches speed up subsequent runs. A newer push cancels older CI runs on the same branch. Release runs are not cancelled while in progress.

JUnit reports and code coverage are available in the `frontend-test-results` artifact for 14 days. Coverage includes panel logic, formatting and release tooling; it does not measure the entire UI. Thresholds: 85% statements/lines, 80% branches and 90% functions.

The desktop test uses timeouts and a separate WebView2 profile. On failure, it saves logs, page text and—if the debugger is reachable—a screenshot to `test-results/desktop/`. CI uploads these files as `windows-desktop-diagnostics`.

In the repository settings for `main`, you can require **CI passed** before merging. This status becomes available after the workflow's first run.

## Local verification

Requirements: Node.js 22.12+ from the 22 release line or Node.js 24 LTS, stable Rust with `rustfmt` and `clippy`, and the Tauri dependencies for your operating system.

```sh
npm ci
npm run format:check
npm run release:check
npm run test:coverage
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked --all-targets
```

Windows integration test:

```sh
npm run tauri build -- --debug --no-bundle --ci --config .github/tauri.smoke.conf.json -- --locked
# PowerShell: $env:MYCMD_SMOKE_DEBUG_PORT = '9222'
# POSIX shell: export MYCMD_SMOKE_DEBUG_PORT=9222
npm run test:desktop
```

`npm test` runs tests without measuring coverage; `npm run test:watch` runs in watch mode. `npm run format` and `cargo fmt --manifest-path src-tauri/Cargo.toml` fix formatting.

## Preparing a version

The version must match in:

- `package.json`,
- `package-lock.json` (the top-level version and root package),
- `src-tauri/tauri.conf.json`,
- `src-tauri/Cargo.toml`,
- `src-tauri/Cargo.lock` (the `mycmd` package only).

The script updates them together without changing dependency versions:

```sh
npm run release:version -- 0.2.0
```

If you use `CHANGELOG.md`, move the relevant entries from `Unreleased` under `## [0.2.0] - YYYY-MM-DD`. A changelog is not required. Run `npm run release:check` and the remaining tests. Commit the changes and merge them into `main` through CI.

For the first release, version `0.1.0` was already prepared and did not need another version bump.

## Starting a release

The tag points to the commit containing the workflow and all release sources. For example, for the first version:

```sh
git switch main
git pull --ff-only
git tag -a v0.1.0 -m "myCmd 0.1.0"
git push origin v0.1.0
```

The **Release** workflow:

1. Checks that the `vX.Y.Z` tag matches the manifests; the changelog is optional.
2. Runs full CI on the tagged commit.
3. Builds packages in parallel:

   | Platform                                     | Architecture        | Packages      |
   | -------------------------------------------- | ------------------- | ------------- |
   | Windows                                      | x64                 | MSI, NSIS EXE |
   | Linux (Debian/Ubuntu; built on Ubuntu 22.04) | x64                 | DEB, AppImage |
   | macOS                                        | Intel x64           | DMG           |
   | macOS                                        | Apple Silicon arm64 | DMG           |

4. Checks that all six installers are present and generates `SHA256SUMS.txt`.
5. Creates a draft GitHub Release and uploads all packages and checksums. The description comes from the changelog, if available, or from default release information.
6. Automatically **publishes the GitHub Release** only after all files have been uploaded.

Intermediate packages are available as workflow artifacts for 14 days; files attached to a GitHub Release do not have this expiration period. The filename prefix identifies the platform and architecture.

Versions with a suffix, such as `0.2.0-rc.1`, are automatically marked as prereleases. Full CI and all platform builds must succeed; failure on any platform blocks publication. DEB packages require Tauri dependencies, including WebKitGTK 4.1; not all older Debian/Ubuntu releases provide them.

The workflow uses the built-in `GITHUB_TOKEN`. Only the final job has `contents: write`. No additional secrets are needed for the current unsigned packages. Windows certificates and Apple signing/notarization are not configured.

## Retrying and troubleshooting

- For test or build failures, open the job logs and corresponding artifacts. Release source fixes under a new version/tag.
- For temporary tool download failures, use **Re-run failed jobs**.
- You can manually run the workflow on an existing tag, for example `gh workflow run release.yml --ref v0.1.0`. Dispatching from a branch is rejected.
- If publication has not completed, a rerun updates the existing draft, replaces its files and then publishes it. A published release will not be overwritten; create a new version instead.

Verify downloaded files on Linux/macOS:

```sh
sha256sum --check SHA256SUMS.txt
# macOS without sha256sum:
shasum -a 256 --check SHA256SUMS.txt
```

On Windows, run `Get-FileHash .\installer-name.exe -Algorithm SHA256` and compare the result with its entry in `SHA256SUMS.txt`.
