<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="apps/mullion/assets/banner-dark.svg">
  <img src="apps/mullion/assets/banner-light.svg" width="640" alt="Mullion">
</picture>

**Well-made parts for the Windows desktop.**

[![Latest release](https://img.shields.io/github/v/release/Aduneer/Mullion?color=%231c1b19)](../../releases/latest)
[![MIT license](https://img.shields.io/badge/license-MIT-1c1b19)](LICENSE)
![Windows 10 and 11](https://img.shields.io/badge/Windows-10%20%C2%B7%2011-1c1b19)
![Built with Rust and Tauri 2](https://img.shields.io/badge/Rust-Tauri%202-9085e9?logo=rust)

</div>

Mullion is a set of small apps, each named after a part of a window (a mullion is the bar between
two panes). Each one is its own program: install the ones you want, quit any of them, and the
rest carry on. They share six palettes, dark or light, solid or glass, so they look like one
desktop. More are on the way: a dock, hot corners, window snapping.

| App | What it is |
| --- | --- |
| [**Fanlight**](apps/fanlight/README.md) | Search and launch on <kbd>Alt</kbd> + <kbd>Space</kbd>: apps, files, settings, windows, sums and what you copied |
| [**Oriel**](apps/oriel/README.md) | A dashboard for your second screen: CPU, GPU, memory, network, disks and processes, what's playing, the weather and your calendar |
| [**Lintel**](apps/lintel/README.md) | A menu bar along the top of the screen: the app in front, your open apps, what's playing, a Control Centre and the clock |
| [**Bay**](apps/bay/README.md) | Every open window at once on <kbd>Win</kbd> + <kbd>Ctrl</kbd> + <kbd>↑</kbd>, as live pictures on each screen: pick one and it comes to the front |

## Install

Download **Mullion-*version*-setup.exe** from the [latest release](../../releases/latest) and tick
the apps you want. It installs just for you, with no admin prompt, and puts Mullion in the Start
menu: one icon in the notification area, one entry in Installed apps and one sign-in entry for
all of them, instead of one of each per app. If you already have one of the apps on its own,
Mullion offers to take its place, and keeps its settings.

The first time Mullion opens, it welcomes you: pick which apps to run and whether Mullion starts
when you sign in. Nothing starts until you press Start, and Oriel waits until you open it.

<picture>
  <source media="(prefers-color-scheme: light)" srcset="apps/mullion/docs/images/settings-light.png">
  <img alt="Mullion Settings: Oriel, Lintel, Fanlight and Bay, each with a switch to run it or not and a way into its own settings, then Start Mullion when I sign in and Quit Mullion" src="apps/mullion/docs/images/settings-dark.png" width="560">
</picture>

Mullion Settings, from its icon, turns each app on or off, and opens each one's own settings.
Each app is still its own program underneath: one crashing or quitting leaves the others running.

Want just one? The same release has each app's own installer and a portable zip. Each app's README
says more, including what to do about SmartScreen and Smart App Control while the releases aren't
signed yet.

## Contributing

Bug reports, ideas and pull requests are welcome: [open an issue](../../issues/new/choose) for a
bug, or start a [discussion](../../discussions) for an idea or a question.
[CONTRIBUTING.md](CONTRIBUTING.md) covers how the repo fits together and making a release; each
app's own CONTRIBUTING covers building and working on it.

## License

[MIT](LICENSE)
