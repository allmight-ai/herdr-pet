# herdr-pet

Companion V-Pet para o [Herdr](https://herdr.dev): uma casinha LCD de 1 bit que espelha o status e a tarefa do seu agente de código.

A espécie, a raridade, o nome e os stats vêm do seu ID do GitHub. Apagar o state e rodar de novo gera o **mesmo** pet — não há reroll.

[English ↓](#english) · [Sponsor](https://github.com/sponsors/allmight-ai)

```
┌──────────────────────────────┐
│ Borixus·9d9b05ea             │
│ Origin · Primordial ✨       │
│ » Fazer isso no 2            │
├──────────────────────────────┤
│             ▀▄▄▀             │
│            ▟▀██▀▙            │
│            ▜▄██▄▛            │
│             ▄▀▀▄             │
│        « treinando »         │
└──────────────────────────────┘
```

## Recursos

- Reage ao `agent_status` do Herdr (treinando, dormindo, curioso, comemorando, confuso)
- Mostra a tarefa atual do agente (`terminal_title`)
- Forja espécie, raridade, shiny, nome e stats a partir do GitHub
- Abre e fecha sob demanda com `prefix+a` em qualquer workspace
- Atalho e CLI no PATH configurados **automaticamente** no install
- Só consome recurso enquanto o painel está aberto

## Instalação

**Requisitos:** [Herdr](https://herdr.dev) ≥ 0.7.4, [Rust](https://rustup.rs) (`cargo` no `PATH`). Linux e macOS.

```bash
herdr plugin install allmight-ai/herdr-pet
```

Pronto. O install:

1. Compila o binário (`cargo build --release`)
2. Grava o atalho no seu `~/.config/herdr/config.toml` (bloco managed)
3. Instala `herdr-pet` em `~/.local/bin` (shim)
4. Recarrega a config do Herdr se o server estiver rodando

**Não precisa editar config à mão.**

| Ação | Como |
| --- | --- |
| Abrir / fechar o pet | `prefix+a` → **Ctrl+b**, soltar, depois **`a`** |
| Redimensionar | `prefix+r` |
| Status no shell | `herdr-pet status` |

Se o atalho não responder depois do install, reinicie o Herdr (ou rode `herdr-pet setup`).

### Desenvolvimento local

```bash
cargo build --release
herdr plugin link .
# ou só o pós-install:
./target/release/herdr-pet setup
```

## Uso

```bash
herdr-pet setup               # reaplicar atalho + PATH (idempotente)
herdr-pet open                # abre ou fecha o painel (precisa do Herdr rodando)
herdr-pet watch               # casinha ao vivo no terminal atual
herdr-pet watch --mood done   # pré-visualiza um humor
herdr-pet status              # dados do pet
herdr-pet gallery             # um pet de cada raridade
herdr-pet init                # trava a âncora do GitHub e choca o pet #0
```

O `watch` inicializa sozinho se ainda não houver state.

### Atalho automático

O Herdr **não** registra teclas a partir do `herdr-plugin.toml` — só a partir do `config.toml` do usuário. Por isso o plugin grava sozinho um bloco managed:

```toml
# >>> herdr-pet (managed — do not edit)
[[keys.command]]
key = "prefix+a"
type = "plugin_action"
command = "allmight-ai.herdr-pet.open"
description = "Pet: toggle (abre/fecha)"
# <<< herdr-pet
```

Se `prefix+a` já estiver ocupado, tenta `prefix+shift+a` e depois `prefix+p`. O `[[startup]]` do plugin re-aplica isso a cada subida do server (útil após update).

## Como funciona

**Forja.**  
`root_seed = HMAC(APP_SALT, github_id)`; cada pet é `HMAC(root_seed, "pet:N")`.  
Dali saem espécie, raridade (60 / 25 / 10 / 4 / 1), shiny (1/128), IVs e nome.  
O mesmo par `(github_id, índice)` sempre produz o mesmo pet.

**Espelho.**  
O `watch` consulta o agente focado (`herdr agent list`) e mapeia o status:

| `agent_status` | humor |
| --- | --- |
| `working` | treinando |
| `done` | comemorando |
| `blocked` | curioso |
| `idle` | dormindo |
| `unknown` | confuso |

A detecção usa o Screen Manifest do Herdr (Claude Code sem configuração extra). O agente precisa estar num painel nativo do Herdr — tmux aninhado quebra a leitura.

**State.**  
Âncora e índice ativo ficam em `HERDR_PLUGIN_STATE_DIR` (no Herdr) ou em `.herdr-pet-state/` (dev).  
A raridade não “mora” no arquivo: é recalculada a partir da âncora.

## Stack e licença

Rust (HMAC-SHA256 + render ANSI) · plugin nativo do Herdr (`herdr-plugin.toml`)

[AGPL-3.0-or-later](LICENSE) · inspirado no petterm (V-Pet LCD 1-bit)

---

## English

A 1-bit LCD V-Pet companion for [Herdr](https://herdr.dev). It mirrors your coding agent’s status and current task.

Species, rarity, name, and stats come from your GitHub ID. Wipe the state and run again — you get the **same** pet. No rerolls.

[Sponsor](https://github.com/sponsors/allmight-ai)

### Features

- Reacts to Herdr `agent_status` (training, sleeping, curious, celebrating, confused)
- Shows the agent’s current task (`terminal_title`)
- Forges species, rarity, shiny, name, and stats from GitHub
- Toggles on demand with `prefix+a` in any workspace
- **Hotkey + CLI PATH are configured automatically on install**
- Runs only while the pane is open

### Install

**Requirements:** [Herdr](https://herdr.dev) ≥ 0.7.4, [Rust](https://rustup.rs) (`cargo` on `PATH`). Linux and macOS.

```bash
herdr plugin install allmight-ai/herdr-pet
```

That’s it. Install builds the binary, writes the hotkey into your Herdr config, puts `herdr-pet` on `~/.local/bin`, and reloads config if the server is running. **No hand-editing required.**

| Action | How |
| --- | --- |
| Toggle the pet | `prefix+a` (Ctrl+b, release, then `a`) |
| Resize | `prefix+r` |
| Shell status | `herdr-pet status` |

Restart Herdr (or run `herdr-pet setup`) if the hotkey does not respond after install.

#### Local development

```bash
cargo build --release
herdr plugin link .
# or just post-install wiring:
./target/release/herdr-pet setup
```

### Usage

```bash
herdr-pet setup               # re-apply hotkey + PATH (idempotent)
herdr-pet open                # toggle the pane (Herdr must be running)
herdr-pet watch               # live house in the current terminal
herdr-pet watch --mood done   # preview a mood
herdr-pet status              # pet data
herdr-pet gallery             # one pet per rarity tier
herdr-pet init                # lock GitHub anchor and hatch pet #0
```

`watch` auto-inits when there is no state yet.

### Automatic hotkey

Herdr does **not** load keybindings from `herdr-plugin.toml` — only from the user’s `config.toml`. The plugin therefore writes a managed block itself (see Portuguese section above). If `prefix+a` is taken, it falls back to `prefix+shift+a`, then `prefix+p`. A `[[startup]]` hook re-applies this on every server start (handy after plugin updates).

### How it works

**Forge.**  
`root_seed = HMAC(APP_SALT, github_id)`; each pet is `HMAC(root_seed, "pet:N")`.  
That seed yields species, rarity (60 / 25 / 10 / 4 / 1), shiny (1/128), IVs, and name.  
The same `(github_id, index)` always produces the same pet.

**Mirror.**  
`watch` polls the focused agent (`herdr agent list`) and maps status to mood. Detection uses Herdr’s Screen Manifest (Claude Code needs no extra config). The agent must run in a native Herdr pane — nested tmux breaks detection.

**State.**  
Anchor and active index live under `HERDR_PLUGIN_STATE_DIR` (Herdr) or `.herdr-pet-state/` (dev).  
Rarity is not stored as ground truth; it is re-derived from the anchor.

### Stack & license

Rust (HMAC-SHA256 + ANSI renderer) · native Herdr plugin (`herdr-plugin.toml`)

[AGPL-3.0-or-later](LICENSE) · inspired by petterm (1-bit LCD V-Pet)
