# 🐾 herdr-pet

> 🇧🇷 Um companion V-Pet para o [**Herdr**](https://herdr.dev) que **espelha o seu agente de código**:
> reage ao status dele (trabalhando / idle / blocked / done) e mostra a tarefa atual, numa
> casinha LCD 1-bit. Leve, sob demanda, em qualquer workspace.
>
> 🇺🇸 A companion V-Pet for [**Herdr**](https://herdr.dev) that **mirrors your coding agent**:
> reacts to its status (working / idle / blocked / done) and shows the current task, in a 1-bit
> LCD house. Lightweight, on-demand, in any workspace.

**Status:** v1 (companion reativo) implementado. A ambição maior — coleção forjada por mérito
(Visão B) — é futuro; veja [`docs/`](docs/).

---

## Demo

```
┌──────────────────────────────┐
│ Borixus·9d9b05ea             │   ← nome + identidade forjada (cor = raridade)
│ Origin · Primordial ✨       │   ← espécie · raridade (✨ shiny)
│ » Fazer isso no 2            │   ← tarefa ATUAL do agente
├──────────────────────────────┤
│             ▀▄▄▀             │
│            ▟▀██▀▙            │   ← sprite 1-bit
│            ▜▄██▄▛            │
│             ▄▀▀▄             │
│        « treinando »         │   ← mood que REAGE ao status do agente
└──────────────────────────────┘
```

## Funcionalidades · Features

- 🪞 **Espelha o agente / Mirrors the agent** — lê o `agent_status` do Herdr e reage com mood
  (*treinando*, *dormindo*, *curioso*, *comemorando*).
- 📋 **Tarefa atual / Current task** — mostra o que o agente está fazendo (`» <título>`).
- 🔐 **Identidade forjada / Forged identity** — espécie + raridade (cor) + shiny derivados do seu
  **ID numérico do GitHub** — **anti-reroll** (re-derivável; a raridade nunca vive só no disco).
- ⚡ **Leve / Lightweight** — pet cacheado, redraw só quando algo muda, e **sob demanda** (só roda
  enquanto o pane está aberto).
- 🌍 **Global** — `prefix+a` abre/fecha em qualquer workspace.

## Instalar · Install

```bash
herdr plugin link .                       # dev local / local dev (rode do diretório do plugin)
# ou / or — de um repo no GitHub / from a GitHub repo:
herdr plugin install FredericoTMello/herdr-pet
```

| Ação · Action            | Atalho · Shortcut                 |
| ---                      | ---                               |
| Abrir/fechar o pet       | `prefix+a`  (Ctrl+b, solta, `a`)  |
| Redimensionar · Resize   | `prefix+r`  (modo resize)         |

> Se o atalho não disparar numa instalação nova, reinicie o Herdr (keybindings carregam no
> startup). / If the hotkey doesn't fire on a fresh install, restart Herdr.

## Uso · Usage

```bash
herdr-pet watch               # ao vivo — espelha o agente / live — mirrors the agent
herdr-pet watch --mood done   # pré-visualiza um mood / preview a mood (dev)
herdr-pet open                # toggle do pane (o que o hotkey chama) / toggles the pane
herdr-pet status              # dados do pet / shows pet data
herdr-pet gallery             # um pet de cada tier / one pet per tier (+ shiny + Primordial)
```

## Como funciona · How it works

- **Forja · Forge**: `root_seed = HMAC(APP_SALT, github_id)` → `pet_seed = HMAC(root_seed, "pet:N")`
  (derivação por sub-seeds, estilo BIP-32). Mesmo `(github_id, índice)` = **mesmo pet, sempre**
  (idempotente — apagar o state e re-rodar nasce o mesmo pet).
- **Espelho · Mirror**: o pane `watch` lê o agente via **socket API do Herdr** (`herdr agent list`)
  e reage ao `agent_status`. Detecção por Screen Manifest (Claude Code: zero-config, latência de
  poucos segundos).

## Stack

- **Rust** — forja (HMAC-SHA256) + renderer LCD (stdout/ANSI, block chars 1-bit).
- **Plugin nativo do Herdr** — manifest `herdr-plugin.toml`; lê o agente via socket API.

## Roadmap

- **v1** ✅ companion reativo (status + tarefa do agente), leve, global.
- **v2 (futuro)** progressão (XP / nível / rebirth estilo Ragnarok) + combate — vai exigir
  anti-cheat (provável: **servidor / open core**).
- **Visão B (futuro aspiracional)** coleção forjada por mérito, gacha / breeding, aura via `gh api`
  — [`docs/ESBOCO-herdr-raridade.md`](docs/ESBOCO-herdr-raridade.md).

## Licença · License

**AGPL-3.0-or-later** — veja [`LICENSE`](LICENSE).

🇧🇷 Cliente open source com copyleft forte: derivados **devem permanecer open** (impede fork
fechado). Monetização planejada via **servidor** futuro (open core: cliente open, servidor
proprietário).
🇺🇸 Strong-copyleft open-source client: derivatives **must stay open** (prevents closed forks).
Monetization planned via a future **server** (open core).

## Créditos · Acknowledgments

Teoria visual e design vêm do **petterm** (V-Pet LCD 1-bit estilo Tamagotchi) — reimplementado em
Rust (referência de design, sem importar código).
