# GitHub and automatic updates

Repository: <https://github.com/tuanxtm/sevi>. Cargo.toml, crates/ and this file
live directly at its root. Only the self-contained Rust workspace is published;
the original repository remains available for upstream comparisons.

## Everyday changes

In a normal clone of the Sevi repository:

```sh
git status
git add <files-you-changed>
git commit -m "Describe the change"
git push origin main
```

The Check workflow tests the desktop code on Windows, Linux and macOS. It also
checks release signing, formatting, typing rules and golden outputs. Review the
Actions results before creating a release tag.

The original checkout still has its Rust workspace in `Rust/`. For that checkout
only, run `python3 Tools/github/export.py` from Rust/ after editing the workspace,
then `git push sevi sevi-main:main` from the original checkout. The export uses a
separate index and branch; it does not check out another tree or stage unrelated
files. Future work is simplest in a normal clone of tuanxtm/sevi.
When releasing from this original checkout, push the exported commit directly to
the new repository's tag, for example `git push sevi sevi-main:refs/tags/v1.0.31`.
This preserves the original repository's local tags, which can have the same names.

## Publish a Windows x64 version

1. Increase `[workspace.package].version` in Cargo.toml, for example to 1.0.31,
   and increase `[workspace.metadata.sevi].build`.
2. Synchronize metadata and the lockfile:

   ```sh
   cargo xtask tool metadata --sync
   cargo update --workspace --offline
   cargo test --locked -p sevi-windows --lib
   ```

3. Commit the version changes and push main. After Check passes, tag that commit:

   ```sh
   git tag -a v1.0.31 -m "Sevi 1.0.31"
   git push origin v1.0.31
   ```

The tag must exactly match the Cargo version. The Release Windows x64 workflow
builds the Rust executable, a per-user installer, and a portable ZIP. It signs the
installer with Sevi's Ed25519 update key, generates appcast.xml and SHA256SUMS,
uploads all files to a draft GitHub release, then publishes the complete release.
Published files cannot be overwritten by rerunning the tool. Use a new version
for corrections; a failed draft can be retried from Actions > Release Windows x64.

The workflow's GITHUB_TOKEN handles release uploads. No personal access token or
website hosting is required. The only additional repository secret is
`SEVI_UPDATE_PRIVATE_KEY`, already configured during this setup.

## What the installed app does

Windows checks the following stable URL:

<https://github.com/tuanxtm/sevi/releases/latest/download/appcast.xml>

With Automatic updates enabled, it checks once a day, downloads a newer signed
installer in the background and installs it on a later restart/exit. Check for
updates in About or the tray menu also supports immediate manual installation.
Both freshly downloaded and cached installers must pass signature verification
before execution. Update downloads run outside the typing engine.

The test executables built before this setup have no Sevi feed and trust a
different public key. Install the first GitHub Setup.exe manually once. It can
then receive later Sevi releases through the new feed. Portable users can use
the same updater; the installer places Sevi in the account's Programs directory.
Quit the old tray instance before this first install. If Start Sevi at login was
enabled in the portable test build, turn it off and on in the newly installed app
to replace the old executable path in the login entry.

These signatures authenticate updates. They are separate from paid Windows
Authenticode certificates; the initial installer is not Authenticode-signed.

## Keep the update key

Only `updates/public-key.txt` is committed. The private backup generated for this
checkout is `.secrets/update-private-key.txt`, with owner-only
permissions and excluded by .gitignore. Keep a separate secure backup. Do not
regenerate the key for routine releases: installed apps trust the existing public
key. GitHub stores the private value encrypted as `SEVI_UPDATE_PRIVATE_KEY`.

To restore that repository secret from the private backup:

```sh
gh secret set SEVI_UPDATE_PRIVATE_KEY --repo tuanxtm/sevi < .secrets/update-private-key.txt
```

Release Python dependencies are build tools only (`Tools/github/requirements.txt`).
They are not bundled with Sevi. Legacy website publishing commands are disabled,
so they cannot publish to the original author's repository or services.

## Other platforms

The same application source and UI remain cross-platform, and Check covers all
three desktop targets. Automated binary publication currently targets Windows
x64, the validated release variant. macOS Sparkle remains disabled until a signed,
notarized Sevi Mac release and Apple credentials are configured; its public key
has been changed to Sevi's key. Linux keeps its existing opt-in release checks
and native package installation. Neither is advertised as an automatic installer
by this Windows-only release workflow.
