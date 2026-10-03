<div align="center">
  <h1>raptor-cage</h1>
  <p>Run native and Windows games in a secure sandbox.</p>
  <img alt="Downloads" src="https://img.shields.io/github/downloads/systemlayer/raptor-cage/total?style=flat-square&label=DOWNLOADS&labelColor=0567ff&color=696969" />
  <img alt="Latest Release" src="https://img.shields.io/github/v/release/systemlayer/raptor-cage?style=flat-square&label=LATEST%20RELEASE&labelColor=0567ff&color=696969" />
  <img alt="AUR" src="https://img.shields.io/aur/version/raptor-cage-bin?style=flat-square&label=AUR&labelColor=0567ff&color=696969" />
</div>

## Why sandbox your games?

- Game developers can make mistakes or overlook security issues.
- Even careful developers can be affected by vulnerabilities in their tools and dependencies, including supply-chain attacks.
- Many games include tracking or data collection, sometimes built into the game engine.

## Installation

### Arch Linux

> ⚠️ Enable the multilib repository in `/etc/pacman.conf` to make 32-bit dependencies available

```bash
# Install with paru.
paru -S raptor-cage-bin

# Alternatively, build and install the AUR package manually.
git clone https://aur.archlinux.org/raptor-cage-bin.git
cd raptor-cage-bin
makepkg -sri
```

### Manual installation

Download the latest release and install the binary as `rcage`:

```bash
download_url="$(curl -sL 'https://api.github.com/repos/systemlayer/raptor-cage/releases/latest' | grep -E 'browser_download_url.+\.tgz' | grep -oP '"browser_download_url": "\K[^"]+')"
curl -L -o raptor-cage.tgz "$download_url"
tar xf raptor-cage.tgz
sudo install -Dm755 rcage "/usr/local/bin/rcage"
```

## Usage

> ⚠️ Network access is denied by default. Use `--network-mode` to change it

### Command line examples

```bash
# Run a Windows game using a runner and prefix from the Bottles data directory.
rcage run -r soda-9.0-1 -p my_prefix -d ~/games/some_game -b game.exe

# Run a native binary with custom arguments after the double dash.
rcage run -d ~/games/some_game -b native_binary -- --param1

# Mount the game directory read-write and the installers read-only, then start a shell.
rcage run -r soda-9.0-1 -p my_prefix -d ~/games/some_game:rw -v ~/installers:/installers

# Use the same mounts to run a Windows installer.
rcage run -r soda-9.0-1 -p my_prefix -d ~/games/some_game:rw -v ~/installers:/installers -b /installers/setup.exe

# Run a Windows launcher, then wait for the game process to exit.
rcage run -r soda-9.0-1 -p my_prefix -d ~/games/some_game -b /usr/bin/rcage -- wait -w '*\Game-Win64.exe' wine -- Launcher.exe

# Use the built-in process-waiting option for the same result.
rcage run -r soda-9.0-1 -p my_prefix -d ~/games/some_game -w '*\Game-Win64.exe' -b Launcher.exe
```

### Placeholders

Use placeholders in game directory (`-d` or `--appdir`) and volume mapping (`-v` or `--volume`) paths with the syntax `{{NAME}}`. Define custom values in the `[placeholders]` section at `rcage.toml`:

```toml
[placeholders]
GAMES_ROOT = "/mnt/games"
SAVES_ROOT = "/mnt/saves"
```

```bash
rcage run -d '{{GAMES_ROOT}}/some_game:rw' -v '{{SAVES_ROOT}}/some_game:/saves:rw' -b game.exe
```

Here, `{{GAMES_ROOT}}/some_game` resolves to `/mnt/games/some_game`. The save directory `/mnt/saves/some_game` is mounted read-write at `/saves` inside the sandbox. Placeholder names are case-sensitive and may contain letters, digits, and underscores. Unknown or malformed placeholders cause the command to fail.

The first existing configuration file is loaded from these locations, in order:

1. `rcage.toml` in the current working directory.
2. `$XDG_CONFIG_HOME/rcage.toml`, when `XDG_CONFIG_HOME` is set.
3. `$HOME/.config/rcage.toml`.

### Run command enum parameters

These options apply to `rcage run`. Aliases shown in parentheses are accepted in place of the full value.

| Option | Values and aliases | Default | Behavior |
| --- | --- | --- | --- |
| `--network-mode` | `full_access` (`full`, `f`), `restricted_access` (`restricted`, `r`), `no_access` (`no`, `n`) | `no_access` | `full_access` allows network access. `restricted_access` hides system DNS configuration and certificate stores, but still allows connections through direct IP addresses. `no_access` isolates the network namespace. |
| `--device-access` | `all` (`a`), `minimal` (`m`) | `minimal` | `all` exposes `/dev`. `minimal` exposes a limited set of devices, including GPUs and game controllers. |
| `--upscale-mode` | `none` (`n`), `dlss` (`d`), `fsr:mode:strength` | `none` | `none` leaves upscaling unconfigured. `dlss` sets NVIDIA DLSS flags. FSR uses the syntax described below. Support depends on the Wine runner. |
| `--sync-mode` | `none`, `fsync`, `esync` | `none` | `none` preserves the runner's synchronization behavior. `fsync` sets `WINEFSYNC=1`; `esync` sets `WINEESYNC=1`. Support depends on the Wine runner. |
| `--display-protocol` | `x11` (`x`), `wayland` (`w`) | `x11` | Configures the display environment and sockets exposed inside the sandbox. |
| `--user-mapping` | `random`, `none`, `UID:GID` | `random` | `random` assigns random user and group IDs. `none` leaves IDs unmapped. `UID:GID` selects explicit IDs, each between `0` and `2147483647`. |

For FSR, use `fsr:mode:strength`, such as `--upscale-mode=fsr:balanced:1`. The mode can be `none` (`n`), `quality` (`q`), `balanced` (`b`), `performance` (`p`), or `ultra` (`u`). Strength must be an integer from `0` to `5`. Use the lowercase `fsr:` prefix. FSR requires a compatible Wine runner and the game to use true fullscreen mode.

### List command enum parameters

Use `rcage list` to list installed Bottles runners and prefixes.

| Option | Values and aliases | Default | Behavior |
| --- | --- | --- | --- |
| `--category` | `all` (`a`), `prefixes` (`p`), `runners` (`r`) | `all` | Lists both categories, only prefixes, or only runners. |

## Frequently asked questions

### How do I enable MangoHud?

Use `-e MANGOHUD=1` for games that use DXVK or VKD3D. OpenGL and WineD3D games may require launching the binary through `mangohud`, for example, `mangohud wine game.exe`.

### How does raptor-cage differ from Bottles?

Bottles provides a GUI for managing Wine/Proton runners, prefixes, and dependencies, and runs under Flatpak. Applications launched by Bottles inherit its [Flatpak permissions](https://github.com/flathub/com.usebottles.bottles/blob/master/com.usebottles.bottles.yml#L9). raptor-cage launches applications with a restricted sandbox by default and lets you adjust their permissions independently.

### Do I need Bottles?

No, but Bottles is recommended for managing Wine/Proton runners and dependencies. You can also download and extract a runner yourself, then select its path with `-r` or `--runner`.

### How does raptor-cage differ from Bubblewrap?

raptor-cage uses Bubblewrap (`bwrap`) to create its sandboxes. You can use Bubblewrap directly, but you would need to configure the mounts, environment, and isolation options yourself.

### Do I need Steam?

Steam is not required to launch games with raptor-cage. The project aims to let you sandbox games without relying on proprietary launchers.

### Why does the Arch Linux package depend on Steam?

The `steam` package provides dependencies needed by Wine/Proton games. The raptor-cage package uses it as a convenient way to install those libraries. To manage the dependencies yourself, install the raptor-cage binary manually instead.

### How do I hide Steam icons on Arch Linux?

To exclude Steam's launchers when installing the dependency, add the following setting to `/etc/pacman.conf`:

```conf
# /etc/pacman.conf
NoExtract = usr/bin/steam usr/bin/steamdeps usr/lib/steam/steam.desktop usr/share/applications/steam.desktop
```

### Do I still need the Steam dependencies on Manjaro?

Manjaro includes more dependencies by default than Arch Linux, but games may still need libraries provided by the `steam` package. Missing dependencies can cause games to freeze or Wine/Proton to report missing libraries such as `libvulkan1.so`.

## Troubleshooting

> See also the [ArchWiki Steam troubleshooting guide](https://wiki.archlinux.org/title/Steam/Troubleshooting#Steam:_An_X_Error_occurred)

### Driver error: nouveau

If you see `Failed to load driver: nouveau`, check that the appropriate 32-bit graphics libraries are installed, such as `lib32-nvidia-utils` for the NVIDIA proprietary driver.

### Wine error: required file not found

Missing 32-bit libraries can cause this error. Libraries bundled with the Bottles Flatpak must also be installed on the host when running a game through raptor-cage. On Arch Linux, installing `wine` or `steam` can help provide the required dependencies.

Some Wine runners use a 32-bit `wine` binary even when running 64-bit applications. Games and installers may also include 32-bit components, so check their dependencies as well.

### Bubblewrap error: permission denied while setting up the UID map

If the command fails with `bwrap: setting up uid map: Permission denied`, AppArmor may be preventing Bubblewrap from creating user namespaces. On systems with this restriction, an AppArmor profile can allow `bwrap` to create them:

```conf
# /etc/apparmor.d/bwrap

abi <abi/4.0>,
include <tunables/global>

profile bwrap /usr/bin/bwrap flags=(unconfined) {
  userns,
  # Site-specific additions and overrides. See local/README for details.
  include if exists <local/bwrap>
}
```

### Shell warning: unknown group ID

The message `groups: cannot find name for group ID ...` can appear on some distributions when sandboxed processes use random user and group IDs. This warning is harmless.

## Development

### Maintenance

```bash
# Check for dependency vulnerabilities.
cargo audit

# Update dependencies within the current version requirements (Cargo.lock).
cargo update

# Check for dependency version updates (Cargo.toml).
cargo upgrade --dry-run
```

### Internal branch-pushing workflow

```bash
# Prune stale remote-tracking branches to avoid branch name collisions.
git remote prune origin

# Push HEAD to another remote branch without changing the upstream.
git push origin HEAD:dev/new-feature
```

### TODOs

* Test with pure 64-bit Wine (see the [Arch Linux transition announcement](https://archlinux.org/news/transition-to-the-new-wow64-wine-and-wine-staging/) and [Wine 9.0 WoW64 notes](https://gitlab.winehq.org/wine/wine/-/releases/wine-9.0#wow64)).
* Implement Bash autocompletion for prefix and runner names detected in Bottles. Consider using [clap_complete](https://crates.io/crates/clap_complete).
* Add an `integrate` subcommand to create integrations, such as `.desktop` shortcuts and Heroic launcher entries.
* Add a `kill` subcommand to terminate all processes in a sandbox by connecting to an existing Bubblewrap container.
* Have the `integrate` subcommand extract the executable's icon for `.desktop` shortcuts, using either a small Windows executable that calls the Win32 API or `wrestool` on Linux.
* Add NTSYNC support (see [Linux 6.14 NTSYNC coverage](https://www.phoronix.com/news/Linux-6.14-Char-Misc-NTSYNC)).
* Add a `--gpu` option (an enum with a default value) to force use of the dedicated GPU. See also:
  * https://wiki.archlinux.org/title/PRIME#Configure_applications_to_render_using_GPU
  * https://download.nvidia.com/XFree86/Linux-x86_64/435.17/README/primerenderoffload.html
  * https://wiki.manjaro.org/index.php/Configure_Graphics_Cards
  * https://wiki.archlinux.org/title/Hybrid_graphics
  * https://wiki.archlinux.org/title/PRIME#Note_about_Windows_games
* Detect the dedicated GPU and enable `--gpu` automatically. Check the `DRI_PRIME`, `__NV_PRIME_RENDER_OFFLOAD`, `__GLX_VENDOR_LIBRARY_NAME`, `__VK_LAYER_NV_optimus`, and `DXVK_FILTER_DEVICE_NAME` variables.
  * Test with `DRI_PRIME=1 glxinfo | grep -E "OpenGL (vendor|renderer)"`. The GPU may initially be powered off; subsequent launches should be faster.
  * The [`prime-run` script](https://gitlab.archlinux.org/archlinux/packaging/packages/nvidia-prime/-/blob/main/prime-run?ref_type=heads) sets these variables.
* Create a `.deb` package that depends on Steam libraries, similar to Arch's `steam` package (see [Ubuntu Steam packages](https://packages.ubuntu.com/search?keywords=steam&searchon=names&suite=noble&section=all)).
* Fork the `steam` package to keep only its dependencies, and add GitHub Actions to check for updates and deploy to the AUR. This would avoid the `pacman.conf` workaround described in the FAQ. The previously used `steam-native-runtime` package was removed from Arch's official packages in early 2026. It included only the needed dependencies, while `steam` includes extras such as `steam-devices` and `zenity`.
* Add an overlay filesystem over the game directory to allow writes without changing the underlying files, as an alternative to `:rw`.

## License

[MIT](https://opensource.org/license/mit)
