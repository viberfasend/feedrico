<!-- Release notes template. scripts/release.mjs fills in the version and changes placeholders
     and drops this comment. Edit the draft release on GitHub before publishing. -->

## Download

| System | File | Portable Mode |
|---|---|---|
| Linux | `Feedrico_{{version}}_amd64.AppImage` | yes |
| Debian, Ubuntu | `Feedrico_{{version}}_amd64.deb` | no |
| Windows 10 and 11 | `Feedrico_{{version}}_x64-setup.exe` (installer) | no |
| Windows 10 and 11 | `Feedrico_{{version}}_x64-portable.exe` | yes |
| macOS 11 or newer (Apple silicon and Intel) | `Feedrico_{{version}}_universal.dmg` | no |

**Portable Mode** keeps app and Library together, for example on a USB stick: put the AppImage
or the portable `.exe` next to a folder named `Feedrico Library`. If the folder is empty,
Feedrico creates your Library in it on the first start; if it holds a Library, Feedrico opens it.

**These builds are not signed yet**, so your system asks before the first start:

- **Windows:** SmartScreen says "Windows protected your PC". Choose *More info*, then *Run anyway*.
- **macOS:** right-click Feedrico in Applications, choose *Open*, then *Open* again. Or allow it
  under *System Settings → Privacy & Security*.
- **Linux AppImage:** make it executable (`chmod +x Feedrico_{{version}}_amd64.AppImage`).

**Updating the `.deb`:** Ubuntu's App Center won't install a newer `.deb` over an older one (it
only shows "Installed"). Install the update from a terminal instead; it replaces the old version
and keeps your Library:

```sh
sudo apt install ./Feedrico_{{version}}_amd64.deb
```

Before opening your Library with a new version, read *Breaking changes* below if there are any:
an upgraded Library won't open in older versions of Feedrico.

## Changes in {{version}}

{{changes}}
