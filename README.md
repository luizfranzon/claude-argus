# argus

Terminal UI that orchestrates Claude Code agent sessions. See `CONTEXT.md` for the domain
model and `crates/argus-tui` for the TUI itself.

## Instalação

### 1. Instalar o Rust (inclui `cargo`)

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Verifique:

```sh
rustc --version
cargo --version
```

### 2. Instalar o Claude Code

Precisa estar disponível no `PATH` como `claude`. Siga o guia oficial:
[claude.com/product/claude-code](https://claude.com/product/claude-code).

### 3. Clonar e compilar

```sh
git clone <repo-url>
cd argus
cargo build
```

## Running

Modo desenvolvimento, a partir do diretório atual:

```sh
cargo run -p argus-tui
```

## Distribuição

Para instalar o binário `argus` no `PATH` (`~/.cargo/bin`), igual ao `claude`:

```sh
cargo install --path crates/argus-tui
```

A partir daí, rodar `argus` em qualquer diretório abre a TUI e cria a workspace
nesse diretório — mesmo comportamento do Claude Code.
