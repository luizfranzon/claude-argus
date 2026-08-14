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

## Troubleshooting

### File watching (rename de sessão, File Explorer, Git panel) não atualiza sozinho

Argus usa `inotify` para observar `~/.claude/sessions` (rename ao vivo) e a
raiz de cada workspace (File Explorer, status do Git). Se o limite de
instâncias `inotify` do seu usuário já estiver esgotado por outros programas
(editores, watchers de build, GitKraken etc.), o `watch()` falha em silêncio
com `Too many open files` (`os error 24`) e Argus continua funcionando, só
sem essas atualizações automáticas.

Confira se bateu o limite:

```sh
cat /proc/sys/fs/inotify/max_user_instances
```

Falhas de watch ficam registradas em `~/.claude/argus-watch-errors.log`.

Para resolver, aumente o limite:

```sh
sudo sysctl fs.inotify.max_user_instances=1024
```

Para persistir entre reboots:

```sh
echo "fs.inotify.max_user_instances=1024" | sudo tee /etc/sysctl.d/99-inotify.conf
sudo sysctl --system
```
