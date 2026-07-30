# Installation

Install `ddl` on your platform:

## macOS (Homebrew)

```bash
brew tap charly-vibes/charly
brew install dulce-de-leche
```

## Linux (binary download)

```bash
curl -fsSL https://github.com/charly-vibes/dulce-de-leche/releases/latest/download/ddl-$(uname -s)-$(uname -m).tar.gz \
  | tar xz
sudo mv ddl /usr/local/bin/
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

Run `ddl init` to bootstrap the entire charly-vibes toolset. See the
[Quick Start](./quick-start.md) guide.