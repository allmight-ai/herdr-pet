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
#0 · Nv 5 · ████████░░ 1200/1500 XP · 1w
```

## Recursos

- Reage ao `agent_status` do Herdr (treinando, dormindo, curioso, comemorando, confuso)
- Mostra a tarefa atual do agente (`terminal_title`)
- Forja espécie, raridade, shiny, nome e stats a partir do GitHub
- Ganha XP e sobe de nível com o trabalho real do agente (curva até o nível 99)
- Ao fechar, mostra um resumo da sessão (agentes, XP, nível, duração)
- Guarda cada sessão num diário e conta sua sequência de dias trabalhados
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

Após clonar, ative o rebuild automático do binário a cada commit/pull — assim o pet
sempre roda o código mais novo (sem precisar lembrar de recompilar à mão):

```bash
git config core.hooksPath githooks   # hooks versionados: post-commit + post-merge
```

## Uso

```bash
herdr-pet setup               # reaplicar atalho + PATH (idempotente)
herdr-pet open                # abre ou fecha o painel (precisa do Herdr rodando)
herdr-pet watch               # casinha ao vivo no terminal atual
herdr-pet watch --mood done   # pré-visualiza um humor (dev, só-leitura)
herdr-pet status              # identidade + XP, nível, sequência e quem está working
herdr-pet log                 # diário: últimos 7 dias e a sequência (--days N)
herdr-pet gallery             # um pet de cada raridade
herdr-pet init                # trava a âncora do GitHub e choca o pet #0
```

O `watch` inicializa sozinho se ainda não houver state. `--mood` é modo dev de pré-visualização: **nunca** grava no state nem ganha XP (o humor é forçado, não trabalho real); sem state, exige `--id` — ex.: `herdr-pet watch --mood done --id 42`.

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
O `watch` agrega **todos** os agentes (`herdr agent list`) e mapeia o status: se qualquer um está `working`, o pet acorda; caso contrário espelha o agente focado. Com vários `working` ao mesmo tempo, ele **rotaciona** entre as tarefas deles (~4 s cada). Também conta subagentes internos ainda rodando (Claude time/Task, Grok `spawn_subagent`). O rodapé mostra quem está working (`⚙ 2 grok, claude`) e `⚙ 0` quando ninguém trabalha.

| `agent_status` | humor |
| --- | --- |
| `working` | treinando |
| `done` | comemorando |
| `blocked` | curioso |
| `idle` | dormindo |
| `unknown` | confuso |

A detecção usa o Screen Manifest do Herdr (Claude Code sem configuração extra). O agente precisa estar num painel nativo do Herdr — tmux aninhado quebra a leitura.

**Progressão (XP e nível).**  
O pet ganha XP só com trabalho **real de qualquer agente** — conta todos os projetos, não só o focado. 1 agente `working` rende o ritmo cheio (~1000 XP/h); cada agente extra rende menos (½, ⅓, … — decaimento harmônico, anti-proliferação). Com o painel fechado, o trabalho é contabilizado na reabertura pelo `state_change_seq`, num ritmo menor. `idle` não rende XP. O nível (1–99) é derivado do XP total — cada nível pede mais que o anterior (`100 × nível`); chegar ao 99 é meta de longo prazo (~1 ano). Ao fechar o pane, um resumo da sessão mostra agentes, XP ganho, nível e duração. Ver `CONTEXT.md` e `docs/adr/`.

**Diário e sequência.**  
Todo fecho de pane vira uma linha em `sessions.jsonl`, ao lado do `state.json`: dia, janela, XP ganho, nível, agentes e tempo acompanhado. `herdr-pet log` soma por dia e mostra a **sequência** — dias consecutivos com trabalho e o recorde; o `status` mostra a mesma linha. O dia é o **local** do fecho, não o UTC: sequência conta os dias de quem trabalhou. Dia sem XP e sem tempo acompanhado não segura a série. O diário é acessório — se a gravação falhar, o pet avisa no fecho e segue inteiro.

**Um dono por vez.**  
Dois `watch` sobre o mesmo `state.json` se sobrescreveriam. A posse é um lock no dir do state: quem abre segundo vira **pet espelho** — desenha tudo, não grava nada (o rodapé marca `⚠ espelho`), e o trabalho do período fica com o dono. Se o dono fechar, o espelho assume no ciclo seguinte, relendo o state do disco.

**State.**  
Âncora e índice ativo ficam em `HERDR_PLUGIN_STATE_DIR` (pane do Herdr) ou no dir XDG do plugin (`~/.local/state/herdr/plugins/allmight-ai.herdr-pet/`) — padrão de leitura **e** escrita fora do pane (`init`/`save` criam); `.herdr-pet-state/` no diretório atual só é usado se **já existir** (compat com dev antigo, nunca criado).  
A gravação é atômica (tmp+rename); se o arquivo ficar ilegível, o conteúdo é preservado em `state.json.corrupt` (com aviso) antes de qualquer recriação — nada se perde em silêncio. A raridade não “mora” no arquivo: é recalculada a partir da âncora.

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
- Earns XP and levels up from the agent's real work (curve up to level 99)
- On close, shows a session summary (agents, XP, level, duration)
- Keeps every session in a journal and tracks your streak of worked days
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

After cloning, enable automatic binary rebuild on every commit/pull — so the pet
always runs the latest code (no need to remember to rebuild by hand):

```bash
git config core.hooksPath githooks   # versioned hooks: post-commit + post-merge
```

### Usage

```bash
herdr-pet setup               # re-apply hotkey + PATH (idempotent)
herdr-pet open                # toggle the pane (Herdr must be running)
herdr-pet watch               # live house in the current terminal
herdr-pet watch --mood done   # preview a mood (dev, read-only)
herdr-pet status              # identity + XP, level, streak, and who is working
herdr-pet log                 # journal: last 7 days and the streak (--days N)
herdr-pet gallery             # one pet per rarity tier
herdr-pet init                # lock GitHub anchor and hatch pet #0
```

`watch` auto-inits when there is no state yet. `--mood` is a read-only dev preview: it **never** writes state or earns XP (the mood is forced, not real work); with no state, pass `--id` — e.g. `herdr-pet watch --mood done --id 42`.

### Automatic hotkey

Herdr does **not** load keybindings from `herdr-plugin.toml` — only from the user’s `config.toml`. The plugin therefore writes a managed block itself (see Portuguese section above). If `prefix+a` is taken, it falls back to `prefix+shift+a`, then `prefix+p`. A `[[startup]]` hook re-applies this on every server start (handy after plugin updates).

### How it works

**Forge.**  
`root_seed = HMAC(APP_SALT, github_id)`; each pet is `HMAC(root_seed, "pet:N")`.  
That seed yields species, rarity (60 / 25 / 10 / 4 / 1), shiny (1/128), IVs, and name.  
The same `(github_id, index)` always produces the same pet.

**Mirror.**  
`watch` aggregates **all** agents (`herdr agent list`) and maps status to mood: if any is `working`, the pet wakes up; otherwise it mirrors the focused one. With several `working` agents at once, it rotates through their tasks (~4 s each). It also counts still-running internal subagents (Claude team/Task, Grok `spawn_subagent` — Herdr only sees the parent process). The footer names who is working (`⚙ 2 grok, claude`) and shows `⚙ 0` when nobody is. Detection uses Herdr’s Screen Manifest (Claude Code needs no extra config). The agent must run in a native Herdr pane — nested tmux breaks detection.

**Progression (XP & level).**  
The pet earns XP only from **any agent's** real work — it counts all projects, not just the focused one. 1 working agent earns the full rate (~1000 XP/h); each extra agent earns less (½, ⅓, … — harmonic decay, anti-proliferation). With the pane closed, work is tallied on reopen via `state_change_seq` at a lower rate. `idle` earns nothing. Level (1–99) is derived from total XP — each level needs more than the last (`100 × level`); reaching 99 is a long-term goal (~1 year). Closing the pane prints a session summary (agents, XP gained, level, duration). See `CONTEXT.md` and `docs/adr/`.

**Journal & streak.**  
Every pane close appends a line to `sessions.jsonl`, next to `state.json`: day, window, XP gained, level, agents, and accompanied time. `herdr-pet log` aggregates per day and shows the **streak** — consecutive worked days plus the record; `status` prints the same line. The day is the **local** date at close, not UTC: a streak counts the days of whoever did the work. A day with no XP and no accompanied time does not hold the streak. The journal is an accessory — if the write fails, the pet says so on close and carries on intact.

**One owner at a time.**  
Two `watch` processes over the same `state.json` would overwrite each other. Ownership is a lock in the state dir: whoever opens second becomes a **mirror pet** — it draws everything and writes nothing (the footer marks `⚠ espelho`), and the work of that period belongs to the owner. If the owner closes, the mirror takes over on the next cycle, re-reading state from disk.

**State.**  
Anchor and active index live under `HERDR_PLUGIN_STATE_DIR` (Herdr pane) or the plugin's XDG dir (`~/.local/state/herdr/plugins/allmight-ai.herdr-pet/`) — the default for reading **and** writing outside the pane (`init`/`save` create it); `.herdr-pet-state/` in the current directory is only honored if it **already exists** (old-dev compat, never created).  
Writes are atomic (tmp+rename); an unreadable file is preserved as `state.json.corrupt` (with a warning) before any re-creation — nothing is lost silently. Rarity is not stored as ground truth; it is re-derived from the anchor.

### Stack & license

Rust (HMAC-SHA256 + ANSI renderer) · native Herdr plugin (`herdr-plugin.toml`)

[AGPL-3.0-or-later](LICENSE) · inspired by petterm (1-bit LCD V-Pet)
