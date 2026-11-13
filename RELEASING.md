# Releasing Riftmeter

Releases are built on your own PC and uploaded to
[GitHub Releases](https://github.com/Lucifer0885/Rift-Meter/releases) by hand. Each build makes
two installers:

- `Riftmeter_<version>_x64-setup.exe`: NSIS installer, installs for the current user, no admin.
- `Riftmeter_<version>_x64_en-US.msi`: MSI installer, installs for all users (needs admin).

Installed copies check
`https://github.com/Lucifer0885/Rift-Meter/releases/latest/download/latest.json` when they start,
and when you click the version number in the app header. If that file names a newer version, the
app offers to download it, install it and restart.

## One-time setup: the signing key

The updater only installs files signed with your private key, so nobody else can push an update
to your users.

1. Generate the key pair (from the `app` folder). It asks for a password:

   ```powershell
   npx tauri signer generate -w "$env:USERPROFILE\.tauri\riftmeter.key"
   ```

2. Open `riftmeter.key.pub` and paste its contents into `app/src-tauri/tauri.conf.json` at
   `plugins.updater.pubkey`, in place of `REPLACE_WITH_PUBLIC_KEY`. The public key is safe to
   commit.
3. Back up `riftmeter.key` and its password somewhere safe, such as a password manager. Never commit
   it. **If you lose it, existing installs can never be updated again**: you'd have to ship a new
   key and everyone would need to reinstall by hand.

## Each release

1. Bump `version` in `app/src-tauri/tauri.conf.json`, for example `0.2.0`. Also set it in
   `app/package.json` to keep the two in step.
2. Load the key into the terminal session (PowerShell):

   ```powershell
   $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.tauri\riftmeter.key" -Raw
   $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = Read-Host "Key password" -MaskInput
   ```

3. Build (from the `app` folder). Optionally pass release notes for the update prompt:

   ```powershell
   npm run release
   # or: npm run release -- "Fixes the party meter"
   ```

   The output is in `app/src-tauri/target/release/bundle/`:
   - `nsis/Riftmeter_0.2.0_x64-setup.exe`
   - `msi/Riftmeter_0.2.0_x64_en-US.msi`
   - `latest.json`

4. Commit the version bump, then create the release on GitHub with the tag `v0.2.0`. The tag must
   be `v` + the version, because `latest.json` links to that tag. Upload these three files:
   the `.exe`, the `.msi` and `latest.json`.

   From the command line, in the `app` folder:

   ```powershell
   $v = "0.2.0"; $b = "src-tauri/target/release/bundle"
   gh release create "v$v" "$b/nsis/Riftmeter_${v}_x64-setup.exe" "$b/msi/Riftmeter_${v}_x64_en-US.msi" "$b/latest.json" --title "Riftmeter v$v" --notes "Install Npcap first: https://npcap.com/#download"
   ```

   Or use **Draft a new release** on the GitHub website and drag the files in.

The updater only sees the newest **published** release. Drafts and pre-releases are skipped, so you
can upload a draft, test the installers, then publish it.

## Notes

- Npcap can't be bundled: its license doesn't allow redistributing the installer for free. Users
  install it themselves from npcap.com.
- The MSI build uses WiX, which Tauri downloads on first use. On recent Windows 11 builds WiX needs
  the optional **VBScript** feature (Settings > System > Optional features). If the MSI step fails
  with a `light.exe` error, enable VBScript and build again.
- The installers aren't code-signed with a Windows certificate, so SmartScreen shows a warning the
  first time users run them. That's separate from the updater key above.
