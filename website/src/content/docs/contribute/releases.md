---
title: Releases and packages
description: Maintainer instructions for semantic releases, native artifacts and package publication.
---

This guide is for project maintainers preparing and publishing a release. To propose a fix or improvement, start with the [contribution guide](/CretSpec/contribute/development/#contributions).

## Release flow

The release workflow validates each commit on `main` and lets Release Please create or update a release PR with Cargo versions and the changelog. Merge that PR to create the tag and GitHub release. Verified archives are uploaded in the same workflow: a release created with `GITHUB_TOKEN` does not trigger a separate release-event workflow. After upload, the workflow rebuilds the documentation with the released download links and deploys it when Pages is enabled.

| Commit | Before 1.0 | From 1.0 |
| --- | --- | --- |
| `fix:` | Patch | Patch |
| `feat:` | Minor | Minor |
| Breaking change (`!` / footer) | Minor | Major |
| Documentation, tests, CI and maintenance only | No release | No release |

Release Please keeps the release manifest and Cargo versions aligned. The installed version comes from Cargo metadata. Tags use `v<version>`. The installation page derives native download URLs from the release manifest, so review its version together with the release artifacts.

## Repository setup

In GitHub Actions settings, allow workflows to create pull requests. The release job requests the required repository permissions. Release Please uses `GITHUB_TOKEN`; no personal token is embedded in the repository.

GitHub does not automatically run ordinary PR workflows for a PR created with this token. Review the release PR and manually run **Verify** on its branch, or close and reopen it with your account to trigger checks. Configure branch protection around the desired review policy before inviting contributions.

## Native artifacts

Verification tests and builds Windows x64, Linux x64, macOS Intel and macOS Apple silicon. Each archive contains its executable, README, MIT license, dependency notices and Rust standard-library licenses. Packaging validates the complete matrix and generates `SHA256SUMS`, a Homebrew formula, WinGet manifests, and AUR `PKGBUILD` / `.SRCINFO` from those exact bytes.

```sh
python -m unittest discover -s scripts -p 'test_*.py'
python scripts/package.py pack --target x86_64-pc-windows-msvc --binary target/release/cspec.exe
python scripts/package.py metadata --artifacts dist --out dist/packages
```

Metadata requires all four archives. Missing or malformed archives fail before package metadata is emitted. Build each binary on its matching platform before packaging.

## Retry a failed upload

Rerun the failed publish job in the original workflow while its artifacts remain available. Uploads compare existing assets byte for byte and refuse replacements. Partial uploads can continue; different bytes are an error.

For manual recovery, check out the release tag, download the original `release-bundle` workflow artifact into `dist/`, authenticate `gh`, set `RELEASE_TAG` to the tag and run `python scripts/publish.py`. Do not rebuild a published version to replace its assets. If original verified bytes are unavailable, investigate and publish a new version.

## Publish package channels

Generated metadata is attached as `cretspec-<version>-packages.zip`. GitHub releases alone do not make a package installable through external catalogs.

To prepare a submission from an existing public release, use the current packaging scripts. This downloads the immutable archives, checks their published SHA-256 hashes and generates the current catalog metadata without replacing any release asset:

```sh
python scripts/prepare-channels.py --version 0.4.0
```

The output is `dist/channels/packages/`. This also applies metadata corrections to the initial 0.4.0 release: current WinGet schema headers and Homebrew formula ordering.

The **Package installation** workflow runs after each release upload and can also be dispatched with a published version. It exercises WinGet validation, unattended installation, command registration, removal and reinstallation on a disposable Windows runner. An empty version selects the latest public release. Its `catalog-submission` artifact contains the tested metadata. This workflow enables local manifests only on that runner and restores the setting afterward. A first release cannot demonstrate an upgrade from an earlier native release; test that separately when the next version is available.

| Channel | Maintainer procedure |
| --- | --- |
| Homebrew | The public [tap](https://github.com/yveslaurentcreton/homebrew-tap) checks releases daily. Its **Packages** workflow verifies downloads, audits and tests the candidate on Intel and Apple silicon macOS, then commits a successful update. Dispatch it manually to update sooner. Failed tests leave the previous formula active. |
| WinGet | Run `winget validate --manifest <version-directory>` and the installation workflow, then submit the three generated manifests for one version to `microsoft/winget-pkgs` from your fork. Keep the PR limited to that version. Catalog review determines availability. Future versions use the same procedure; automatic cross-repository submission is not configured. |
| AUR | Use the maintainer's AUR account and SSH key. Validate/build on Arch Linux, compare `makepkg --printsrcinfo` with `.SRCINFO`, install and test `cspec`. Commit and push to `cretspec-bin` after verifying ownership and the public download. |

Before first submission, confirm names are available and downloads work anonymously. Never add registry credentials to product files. Update the installation page when a channel is actually usable.

See [Release Please](https://github.com/googleapis/release-please), [WinGet contributions](https://learn.microsoft.com/en-us/windows/package-manager/package/repository), [Homebrew taps](https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap) and [PKGBUILD](https://man.archlinux.org/man/PKGBUILD.5.en) for upstream processes.
