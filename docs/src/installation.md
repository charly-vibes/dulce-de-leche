# Installation

Install `ddl` on your platform. All platforms have a **prebuilt binary**
option (no toolchain needed); package managers and `cargo` are alternatives.

| Platform | Fastest path | Alternatives |
|----------|--------------|--------------|
| macOS | Homebrew (below) or the curl one-liner | `cargo install dulce-de-leche` |
| Linux | curl one-liner (below) | Homebrew, `cargo install dulce-de-leche` |
| Windows | Scoop (below) | `cargo install dulce-de-leche` |

## macOS & Linux (curl — prebuilt binary, resolves the latest release)

```bash
V=$(basename "$(curl -fsSLI -o /dev/null -w '%{url_effective}' \
  https://github.com/charly-vibes/dulce-de-leche/releases/latest)" | sed 's/^v//')
TGT="$(uname -s | tr '[:upper:]' '[:lower:]')_$(uname -m | sed 's/^x86_64$/amd64/; s/^aarch64$/arm64/')"
curl -fsSL "https://github.com/charly-vibes/dulce-de-leche/releases/download/v${V}/ddl_${V}_${TGT}.tar.gz" | tar xz
chmod +x ddl && sudo mv ddl /usr/local/bin/
```

## macOS (Homebrew)

```bash
brew tap charly-vibes/charly
brew install dulce-de-leche
```

## Windows (Scoop)

```powershell
scoop bucket add charly https://github.com/charly-vibes/scoop-charly.git
scoop install dulce-de-leche
```

## Any platform (Cargo)

```bash
cargo install dulce-de-leche
```

## Verify installation

```bash
ddl --version
```

## Next steps

Run `ddl init` to bootstrap the entire charly-vibes toolset — see the
[Quick Start](./quick-start.md) guide. If the install fails, see
[Troubleshooting](./troubleshooting.md).
