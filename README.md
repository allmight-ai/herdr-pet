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
- Só consome recurso enquanto o painel está aberto

## Instalação

**Requisitos:** [Herdr](https://herdr.dev) ≥ 0.7.4, [Rust](https://rustup.rs) (`cargo` no `PATH`). Linux e macOS.

```bash
# Pelo GitHub (o install compila com cargo)
herdr plugin install allmight-ai/herdr-pet

# Desenvolvimento local (o link não compila sozinho)
cargo build --release
herdr plugin link .
```

| Ação | Atalho |
| --- | --- |
| Abrir / fechar o pet | `prefix+a` (Ctrl+b, soltar, depois `a`) |
| Redimensionar | `prefix+r` |

Se o atalho não funcionar depois da instalação, reinicie o Herdr.

## Uso

```bash
herdr-pet watch               # casinha ao vivo
herdr-pet watch --mood done   # pré-visualiza um humor
herdr-pet open                # abre ou fecha o painel (atalho)
herdr-pet status              # dados do pet
herdr-pet gallery             # um pet de cada raridade
herdr-pet init                # trava a âncora do GitHub e choca o pet #0
```

O `watch` inicializa sozinho se ainda não houver state.

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
- Runs only while the pane is open

### Install

**Requirements:** [Herdr](https://herdr.dev) ≥ 0.7.4, [Rust](https://rustup.rs) (`cargo` on `PATH`). Linux and macOS.

```bash
# From GitHub (install runs cargo build --release)
herdr plugin install allmight-ai/herdr-pet

# Local development (link does not build for you)
cargo build --release
herdr plugin link .
```

| Action | Shortcut |
| --- | --- |
| Toggle the pet | `prefix+a` (Ctrl+b, release, then `a`) |
| Resize | `prefix+r` |

Restart Herdr if the hotkey does not work after install.

### Usage

```bash
herdr-pet watch               # live house
herdr-pet watch --mood done   # preview a mood
herdr-pet open                # toggle the pane (hotkey target)
herdr-pet status              # pet data
herdr-pet gallery             # one pet per rarity tier
herdr-pet init                # lock GitHub anchor and hatch pet #0
```

`watch` auto-inits when there is no state yet.

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
