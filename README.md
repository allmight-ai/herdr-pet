# herdr-pet

Companion V-Pet para o [Herdr](https://herdr.dev). Espelha o status do seu agente de código e mostra a tarefa atual numa casinha LCD 1-bit. A raridade do pet é forjada a partir do seu ID do GitHub — apagar o state e rodar de novo gera o mesmo pet.

[English](#english)

## Demonstração

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

## O que faz

- Reage ao `agent_status` do Herdr (treinando, dormindo, curioso, comemorando, confuso).
- Mostra a tarefa atual do agente (`terminal_title`).
- Forja espécie, raridade, shiny, nome e stats a partir do ID do GitHub.
- Abre e fecha sob demanda com `prefix+a` em qualquer workspace.
- Só roda enquanto o pane está aberto; redesenha quando algo muda.

## Instalar

```bash
herdr plugin link .                         # desenvolvimento local
herdr plugin install allmight-ai/herdr-pet  # do GitHub
```

| Ação | Atalho |
| --- | --- |
| Abrir / fechar o pet | `prefix+a` (Ctrl+b, solta, `a`) |
| Redimensionar | `prefix+r` |

Se o atalho não responder depois de instalar, reinicie o Herdr.

## Uso

```bash
herdr-pet watch               # casinha ao vivo
herdr-pet watch --mood done   # pré-visualiza um mood
herdr-pet open                # abre/fecha o pane (o que o atalho chama)
herdr-pet status              # dados do pet
herdr-pet gallery             # um pet de cada tier
herdr-pet init                # trava a âncora GitHub e choca o pet #0
```

O `watch` faz auto-init se ainda não houver state.

## Como funciona

**Forja.** `root_seed = HMAC(APP_SALT, github_id)`; cada pet é `HMAC(root_seed, "pet:N")`. Do seed saem espécie, raridade (60/25/10/4/1), shiny (1/128), IVs e nome. O mesmo par `(github_id, índice)` sempre produz o mesmo pet.

**Espelho.** O pane `watch` lê o agente focado via socket API do Herdr (`herdr agent list`) e mapeia o status:

| `agent_status` | mood |
| --- | --- |
| `working` | treinando |
| `done` | comemorando |
| `blocked` | curioso |
| `idle` | dormindo |
| `unknown` | confuso |

A detecção usa o Screen Manifest do Herdr (Claude Code sem config extra). O agente precisa rodar num painel nativo do Herdr — tmux aninhado quebra a leitura.

**State.** Âncora e índice ativo ficam em `HERDR_PLUGIN_STATE_DIR` (no Herdr) ou em `.herdr-pet-state/` (dev). A raridade não depende do arquivo: ela é rederivada da âncora.

## Stack

Rust (HMAC-SHA256 + renderer ANSI) e plugin nativo do Herdr (`herdr-plugin.toml`).

## Licença

[AGPL-3.0-or-later](LICENSE).

Inspirado no petterm (V-Pet LCD 1-bit).

---

## English

A V-Pet companion for [Herdr](https://herdr.dev). It mirrors your coding agent's status and current task in a 1-bit LCD house. The pet's rarity is forged from your GitHub ID — wipe the state and re-run, and you get the same pet.

### Features

- Reacts to Herdr's `agent_status` (training, sleeping, curious, celebrating, confused).
- Shows the agent's current task (`terminal_title`).
- Forges species, rarity, shiny, name, and stats from your GitHub ID.
- Toggles on demand with `prefix+a` in any workspace.
- Runs only while the pane is open; redraws when something changes.

### Install

```bash
herdr plugin link .                         # local development
herdr plugin install allmight-ai/herdr-pet  # from GitHub
```

| Action | Shortcut |
| --- | --- |
| Toggle the pet | `prefix+a` (Ctrl+b, release, `a`) |
| Resize | `prefix+r` |

Restart Herdr if the hotkey does not fire after install.

### Usage

```bash
herdr-pet watch               # live house
herdr-pet watch --mood done   # preview a mood
herdr-pet open                # toggle the pane (what the hotkey runs)
herdr-pet status              # pet data
herdr-pet gallery             # one pet per tier
herdr-pet init                # lock GitHub anchor and hatch pet #0
```

`watch` auto-inits when there is no state yet.

### How it works

**Forge.** `root_seed = HMAC(APP_SALT, github_id)`; each pet is `HMAC(root_seed, "pet:N")`. Species, rarity (60/25/10/4/1), shiny (1/128), IVs, and name come from that seed. The same `(github_id, index)` always yields the same pet.

**Mirror.** The `watch` pane reads the focused agent through Herdr's socket API (`herdr agent list`) and maps status to mood. Detection uses Herdr's Screen Manifest (Claude Code needs no extra config). The agent must run in a native Herdr pane — nested tmux breaks detection.

**State.** Anchor and active index live under `HERDR_PLUGIN_STATE_DIR` (Herdr) or `.herdr-pet-state/` (dev). Rarity is not stored as truth on disk; it is re-derived from the anchor.

### Stack

Rust (HMAC-SHA256 + ANSI renderer) and a native Herdr plugin (`herdr-plugin.toml`).

### License

[AGPL-3.0-or-later](LICENSE).

Inspired by petterm (1-bit LCD V-Pet).
