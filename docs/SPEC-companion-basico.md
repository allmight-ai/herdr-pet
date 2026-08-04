# SPEC — herdr-pet: companion básico

**Status:** SPEC (decisões fechadas em sessão de grilling, 2026-08-04). Base pra implementação.
**Escopo:** o companion **básico** — MVP local. **Não** é a Visão B (coleção) do
[`ESBOCO-herdr-raridade.md`](./ESBOCO-herdr-raridade.md).

## Visão (uma frase)

Um companion de **1 pet** que **espelha o status da sessão do agente** (animações), **fica mais
forte com o seu trabalho** (progressão), com **poucos stats** e **combate simples**. Um amigo que
acompanha o programador em cima da sessão do agente.

## Relação com o ESBOCO

O `ESBOCO-herdr-raridade.md` descreve a Visão B ambiciosa (coleção forjada por mérito, aura via
`gh api`, gacha, breeding, IVs). **Este spec descreve o que se constrói AGORA.** A Visão B passa a
**futuro aspiracional**, não escopo corrente.

- **Reaproveita:** a **forja determinística por âncora GitHub** (anti-reroll da identidade) — mas
  agora só pra determinar o visual.
- **Fica de fora (por enquanto):** coleção, gacha, breeding, aura-gh-api, IVs, profundidade Pokémon.

## 1. Input: status do agente (fato do Herdr)

O pet reage ao **estado da sessão do agente**, inferido pelo Herdr via **Screen Manifest TOML Rules**
(Claude Code: zero config, alta precisão, latência de poucos segundos).

- **Restrição dura:** o agente deve rodar em **painel nativo do Herdr** — tmux aninhado quebra a
  detecção do buffer do PTY (pré-requisito do companion funcionar).
- Latência de poucos segundos é aceitável — dá sensação relaxada de V-Pet.
- **Enum real de estados** (`herdr agent wait --until`, confirmado no Herdr 0.8.0):
  **`working`, `done`, `blocked`, `idle`, `unknown`** — mapeados em §3. Não há estado de "erro"
  distinto (um agente que erra vira `idle`/`blocked`/`unknown`).
- **Mecanismo (plugin):** o pane `watch` lê o `agent_status` do agente associado ao seu
  workspace/pane via **socket API** (`herdr api snapshot` / `herdr agent get`, usando
  `HERDR_SOCKET_PATH`). Poll leve ou reativo a eventos. O campo `state_change_seq` detecta
  transições (ex.: `working → done` dispara o spike). *(Confirmar no build se
  `HERDR_PLUGIN_CONTEXT_JSON` já carrega `agent_status` — se sim, fica de graça na invocação.)*

## 2. Stats

- **3 stats de combate: `HP`, `ATK`, `DEF`.** (Enxuga `sp`, `sp_atk`, `sp_def`, `speed` do modelo
  atual.)
- **Força vem do trabalho (nível), não de genes forjados.** Tese: *"trabalho = força."*
- **A forja determinística vira só cosmético:** determina **espécie + cor de raridade + shiny**
  (identidade visual), **nunca** a força.
- **Sem IVs no MVP** (camada futura, se voltar à profundidade Pokémon).

## 3. Reatividade + progressão (mesmo sinal)

O **mesmo sinal** (status do agente) drive a **animação** E o **XP**.

| `agent_status` | Pet (animação)        | XP                 |
| ---            | ---                   | ---                |
| `working`      | treinando, energizado | 100/min            |
| `done`         | comemorando           | +500 (spike na conclusão) |
| `blocked`      | alerta, curioso       | 15/min             |
| `idle`         | dormindo              | 5/min              |
| `unknown`      | confuso/neutro        | 0 (detecção incerta — não recompensa) |

- **Tone:** companion querido (`blocked` = curioso, não impaciente; `unknown` = confuso, não triste).
- **Ranking de XP:** `working` ≫ `done`-spike > `blocked` > `idle` > `unknown`(=0). (Sem estado de
  "erro" no Herdr; `unknown` = detecção incerta, não dá XP pra não recompensar falha de detecção.)
- **Curva de nível:** polinomial acelerante até **nível 99** (`xp_próx_nível(N) ≈ k·N^1.4`, `k`
  centralizado e tunável).
- **Rebirth (estilo Ragnarok) no 99:** reset nível/XP → 1, **+1 renascimento**, e por renascimento:
  - **+teto de stat** (transcendente — cresce além do cap original),
  - **+marcador visual** (estrela/aura = nº de renascimentos),
  - **+%XP** (regrow mais rápido a cada ciclo),
  - com **retornos decrescentes** por renascimento (o bônus de stat e de %XP caem a cada ciclo) pra
    evitar runaway.

## 4. Combate (simples)

- **Auto-duel turn-based vs inimigos gerados** — temáticos e fofos (bug, merge conflict, failing test
  como "espécies" inimigas).
- **Dano = `ATK_atacante − DEF_defensor` (mín 1).** HP → 0 perde.
- **Recompensa = rank/arena ou cosmético — NÃO XP.** (XP já vem do trabalho; combate dar XP seria
  circular.) Combate é a ***finalidade*** do poder que você moeu: working → nível → ganha lutas →
  rank/cosmético.

## Segurança / anti-cheat (honesto)

- Stats de gameplay (XP, nível, HP/ATK/DEF atuais, rebirth) são **condição local → editáveis**. Sem
  servidor, **não há como blindar totalmente** um app local open-source — quem controla a máquina
  controla o state. (Por isso a *identidade* é ancorada no GitHub.)
- **Pro companion básico isso é aceitável:** single-player, **sem stake** — trapacear só afeta o
  próprio jogador. A única coisa com "valor" é a **identidade cosmética forjada**, e essa **já** é
  ancorada/anti-reroll.
- **Anti-cheat pesado** (verificação server-side, ou derivar progressão de `gh api`) **fica pra
  era-servidor** — quando houver stake compartilhado.
- (Opcional, barato e fraco: assinar o state local com HMAC+salt embarcado — freia edição casual,
  quebrável por quem lê o fonte. Não essencial pro stakeless.)

## Fora de escopo (futuro)

- **Boss mundial / era multiplayer / servidor** — precisa de autoridade server-side pra anti-cheat;
  "sem cheaters" e "sem servidor" colidem. Único caminho sem servidor próprio: **GitHub-as-server**
  (boss via repo/gist + auth GitHub) — hacky, suscetível a automação; exploração futura.
- **Coleção / gacha / breeding / aura via gh-api / IVs / profundidade Pokémon** — Visão B
  aspiracional; retornam como esforço próprio quando o básico amadurecer.
- **PvP / trading / leaderboard** — mesmo motivo (precisa servidor).

## Notas de implementação

- **Config centralizada:** todos os números tunáveis (rates de XP por status, `k` da curva, bônus de
  rebirth, fórmula de dano) numa **única struct/const** — retunar é editar num lugar, **sem bump de
  `genesis_version`** (é condição, não identidade).
- **Reaproveitar:** `forge.rs` (forja cosmética), `render.rs`/`sprites.rs` (animações LCD),
  `crypto.rs` (genes/âncora).
- **State novo em `state.rs`:** nível, XP, HP/ATK/DEF atuais, rebirth count (+ leitura do status do
  agente via contexto/eventos do Herdr).
- **Enxugar/remover do path MVP:** `IV`, o split `sp_atk`/`sp_def`, `BaseStats` pesado por tier (vira
  simples, ou por espécie cosmética). O código removido não é lixo — é a camada "se voltarmos à
  profundidade Pokémon".

## Próximos passos (ordem de build sugerida)

1. **State de gameplay** (nível, XP, stats atuais) + **config centralizada**.
2. **Leitura do status do agente** (contexto/eventos do Herdr) → XP + trigger de animação.
3. **Animações de status** no renderer LCD.
4. **Curva de nível + rebirth.**
5. **Combate simples** (auto-duel + rank/cosmético).
