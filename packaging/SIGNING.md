# Signing the Windows downloads

Unsigned, the Windows installer and `.exe` get SmartScreen's "Windows protected your PC" warning. Modular uses [SignPath Foundation](https://signpath.org/)'s free signing for open-source projects for now: the publisher shows as **SignPath Foundation**. Once Modular has real users, the plan is Azure Artifact Signing ($9.99/month), which signs as Chris.

`release.yml` already has the signing steps. They're skipped until the repository variable `SIGNPATH_ORGANIZATION_ID` is set, so releases build unsigned until then.

## Setting it up

1. **Apply** at [signpath.org/apply](https://signpath.org/apply). Modular qualifies: a public MIT repository with releases. The application is reviewed by hand and takes a while.
2. Once accepted, SignPath sets up a project. In it:
   - Add a **trusted build system**: GitHub.com, linked to `chrischaps/Modular`.
   - Add an **artifact configuration** that signs a `.exe` at the root of the artifact (one zip with one PE file). The same one serves both requests: the app's executable, and the installer.
   - Note the **project slug** and the **signing policy slug** (`release-signing` is assumed).
3. In GitHub, under **Settings → Secrets and variables → Actions**:
   - Secret `SIGNPATH_API_TOKEN`: a CI user's API token from SignPath.
   - Variable `SIGNPATH_ORGANIZATION_ID`.
   - Variables `SIGNPATH_PROJECT_SLUG` and `SIGNPATH_POLICY_SLUG`, if they aren't `modular` and `release-signing`.
4. Try it without publishing: **Actions → Build and Release → Run workflow** builds and signs everything as workflow artifacts. Check a downloaded `.exe` with PowerShell: `Get-AuthenticodeSignature .\modular_synth.exe` should say `Valid`, signed by SignPath Foundation.

Each Windows build makes two requests: the executable first, then the installer that contains it. The ASIO build is signed too; a GPLv3 binary still qualifies.

## What the app does with it

The in-app updater (`src/app/update/signature.rs`) reads both signatures before swapping. Once a copy is signed, it only installs an update signed by one of `PUBLISHERS`: SignPath Foundation, or Chris Chappelear for after the switch to Azure. Keep both names there across that switch, so either kind of release can update to the other. While releases are unsigned, unsigned updates are accepted, still checked against `SHA256SUMS`.

## macOS

Unsigned for now: the first open is right-click → **Open**, and the in-app updater offers **Download** instead of **Install**, because macOS quarantines downloaded apps. A Developer ID certificate and notarization ($99/yr) would fix both.
