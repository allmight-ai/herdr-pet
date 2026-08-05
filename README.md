# 🐾 herdr-pet

Companion V-Pet para o [Herdr](https://herdr.dev) que espelha o seu agente de código: reage ao
status dele (trabalhando / idle / blocked / done) e mostra a tarefa atual, numa casinha LCD 1-bit.
Leve, sob demanda, em qualquer workspace. A raridade do pet é forjada a partir do seu ID do GitHub.

> v1 implementado. A coleção forjada por mérito (Visão B) é futuro — ver [`docs/`](docs/).
> [English](#english)

---

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

## Funcionalidades

- **Espelha o agente** — lê o `agent_status` do Herdr e reage: treinando, dormindo, curioso, comemorando.
- **Tarefa atual** — mostra o que o agente está fazendo.
- **Identidade forjada** — espécie, raridade e shiny derivados do seu ID do GitHub. Anti-reroll: apagar o state e re-rodar nasce o mesmo pet.
- **Leve** — pet cacheado, só redesenha quando algo muda, e roda apenas enquanto o pane está aberto.
- **Global** — `prefix+a` abre e fecha o pet em qualquer workspace.

## Instalar

```bash
herdr plugin link .                              # desenvolvimento local
herdr plugin install FredericoTMello/herdr-pet   # do GitHub
```

| Ação | Atalho |
| --- | --- |
| Abrir / fechar o pet | `prefix+a` (Ctrl+b, solta, `a`) |
| Redimensionar | `prefix+r` |

Se o atalho não disparar depois de instalar, reinicie o Herdr.

## Uso

```bash
herdr-pet watch               # casinha ao vivo (espelha o agente)
herdr-pet watch --mood done   # pré-visualiza um mood
herdr-pet open                # abre/fecha o pane (o que o atalho chama)
herdr-pet status              # dados do pet
herdr-pet gallery             # um pet de cada tier
```

## Como funciona

- **Forja** — `root_seed = HMAC(APP_SALT, github_id)`; cada pet por `HMAC(root_seed, "pet:N")` (derivação por sub-seeds, estilo BIP-32). O mesmo `(github_id, índice)` gera sempre o mesmo pet.
- **Espelho** — o pane `watch` lê o agente pela socket API do Herdr (`herdr agent list`) e reage ao `agent_status`.

## Stack

- **Rust** — forja (HMAC-SHA256) + renderer LCD (stdout/ANSI).
- **Plugin nativo do Herdr** — `herdr-plugin.toml`.

## Roadmap

- **v1** ✅ companion reativo (status + tarefa do agente).
- **v2** — progressão (XP, nível, rebirth) e combate.
- **Visão B** — coleção forjada por mérito, gacha, breeding ([`docs/`](docs/)).

## Licença

AGPL-3.0-or-later. Veja [`LICENSE`](LICENSE).

---

## English

A companion V-Pet for [Herdr](https://herdr.dev) that mirrors your coding agent: it reacts to the
agent's status (working / idle / blocked / done) and shows the current task, in a 1-bit LCD house.
Lightweight, on-demand, in any workspace. The pet's rarity is forged from your GitHub ID.

> v1 implemented. The forged-collection vision (Visão B) is future — see [`docs/`](docs/).

### Features

- **Mirrors the agent** — reads Herdr's `agent_status` and reacts: training, sleeping, curious, celebrating.
- **Current task** — shows what the agent is doing.
- **Forged identity** — species, rarity, and shiny derived from your GitHub ID. Anti-reroll: wipe the state and re-run yields the same pet.
- **Lightweight** — cached, redraws only on change, runs only while the pane is open.
- **Global** — `prefix+a` toggles the pet in any workspace.

### Install

```bash
herdr plugin link .                              # local dev
herdr plugin install FredericoTMello/herdr-pet   # from GitHub
```

| Action | Shortcut |
| --- | --- |
| Toggle the pet | `prefix+a` (Ctrl+b, release, `a`) |
| Resize | `prefix+r` |

If the hotkey doesn't fire after installing, restart Herdr.

### Usage

```bash
herdr-pet watch               # live house (mirrors the agent)
herdr-pet watch --mood done   # preview a mood
herdr-pet open                # toggles the pane (what the hotkey calls)
herdr-pet status              # pet data
herdr-pet gallery             # one pet per tier
```

### How it works

- **Forge** — `root_seed = HMAC(APP_SALT, github_id)`; each pet via `HMAC(root_seed, "pet:N")` (BIP-32-style sub-seed derivation). The same `(github_id, index)` always yields the same pet.
- **Mirror** — the `watch` pane reads the agent via Herdr's socket API (`herdr agent list`) and reacts to `agent_status`.

### Stack

- **Rust** — forge (HMAC-SHA256) + LCD renderer (stdout/ANSI).
- **Native Herdr plugin** — `herdr-plugin.toml`.

### Roadmap

- **v1** ✅ reactive companion (agent status + task).
- **v2** — progression (XP, level, rebirth) and combat.
- **Visão B** — forged collection, gacha, breeding ([`docs/`](docs/)).

### License

AGPL-3.0-or-later. See [`LICENSE`](LICENSE).

---

Design inspirado no **petterm** (V-Pet LCD 1-bit estilo Tamagotchi).
