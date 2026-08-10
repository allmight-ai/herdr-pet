# herdr-pet — design

O que o plugin faz e por quê. Para uso e instalação, veja o [README](../README.md).

## Ideia

Um pet de um slot que acompanha o programador no Herdr: reage ao status do agente e mostra a tarefa atual, numa casinha LCD. A identidade visual (espécie, raridade, shiny, nome) vem da conta GitHub, não de sorte local.

## Identidade forjada

- Âncora = ID numérico do GitHub (`gh api user --jq .id`), imutável e fora da máquina.
- `root_seed = HMAC(APP_SALT, github_id)`; pet `N` = `HMAC(root_seed, "pet:N")` (sub-seeds, estilo BIP-32).
- Genes nomeados (`species`, `rarity`, `shiny`, `iv`, …): cada um é `HMAC(seed, nome)`.
- Raridade: Common 60 · Uncommon 25 · Rare 10 · Epic 4 · Legendary 1. Shiny 1/128.
- O mesmo `(github_id, índice)` sempre gera o mesmo pet. Apagar o state não permite reroll.

O state no disco guarda só âncora, índice ativo e o que já chocamos. A raridade é rederivada.

## Stats

Seis IVs (0–31) no padrão Pokémon e stats de combate: HP, SP, ATK, DEF, SpA, SpD, SPE. Base por tier + IV. Hoje são identidade cosmética forjada — a força efetiva passa a derivar do nível (ver Progressão; o combate que liga isso é a próxima fase).

## Progressão (XP e nível)

O pet ganha XP só com **trabalho real** do agente — nunca de tempo idle:

- **Acompanhado** (painel aberto + agente `working`): ritmo cheio, ~1000 XP/h.
- **Não acompanhado** (painel fechado): contabilizado na reabertura pelo delta do `state_change_seq`, num ritmo menor.

`idle` rende 0 XP (anti-cheat: o sinal de trabalho vem do Herdr, não de arquivo local editável). O nível (1–99) é **derivado** do XP total; cada nível custa `100 × nível` (acelerante); nível 99 ≈ 485.100 XP ≈ 1 ano de uso. Decisão: `docs/adr/0001-xp-from-real-agent-work.md`; linguagem: `CONTEXT.md`.

## Espelho do agente

O `watch` polla o agente focado (`herdr agent list`) e anima o mood:

| `agent_status` | mood |
| --- | --- |
| `working` | treinando |
| `done` | comemorando |
| `blocked` | curioso |
| `idle` | dormindo |
| `unknown` | confuso |

Também exibe `terminal_title`. Detecção via Screen Manifest do Herdr; o agente precisa estar num pane nativo (sem tmux aninhado).

## Deployment

Não há pane global no Herdr. O pet abre sob demanda: a action `open` faz toggle de um split pequeno com o entrypoint `lcd` (`watch`). O processo só existe enquanto o pane está aberto.

O Herdr **não** carrega `[[keys.command]]` do manifesto do plugin — só do `config.toml` do usuário. Também não coloca o binário no PATH. Por isso o install precisa ser zero-config:

1. `[[build]]` → `scripts/build.sh` → `cargo build --release` + `herdr-pet setup`
2. `setup` grava um bloco managed no `~/.config/herdr/config.toml` (atalho `prefix+a`, com fallback) e um shim em `~/.local/bin/herdr-pet`
3. `[[startup]]` re-roda `setup --quiet` a cada subida do server (path do plugin muda no update)

Atalho padrão: `prefix+a` → action `allmight-ai.herdr-pet.open`.

## Stack e contratos

- Rust, plugin nativo (`herdr-plugin.toml`), id `allmight-ai.herdr-pet`.
- `genesis_version` na proveniência do pet: nascimentos antigos continuam rederiváveis com a versão em que nasceram.
- Licença AGPL-3.0-or-later.
