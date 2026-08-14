# Caça de bugs orquestrada — como o Herdr e a IA controlaram tudo

**Data:** 2026-08-14 · **Alvo:** herdr-pet v0.3.1 · **Resultado:** 26 commits na
`fix/p0-bug-hunt`, suíte de 81 → 127 testes, 3 P0 + 7 P1 + os P2/P3 vivos do
veredito, todos consertados e sobreviventes a 7 rodadas de revisão adversarial.

Este documento registra o **processo**, não os bugs (esses estão no histórico de
commits, um por um). A caça inteira foi conduzida de um único prompt de chat,
usando o Herdr como plano de controle de uma equipe de IAs.

## O elenco

| papel | quem | onde |
|---|---|---|
| **Orquestrador** | Claude (Fable) | sessão principal do Claude Code |
| **Caçador de progressão** | Grok (`grok-xp`) | pane Herdr `w19:pR` |
| **Caçador de detecção** | Grok (`grok-detect`) | pane Herdr `w19:pT` |
| **Caçador de persistência** | GLM via Claude Code (`glm-state`) | pane Herdr `w19:pS` |
| **Caçador de watch/UI** | GLM via Claude Code (`glm-watch`) | pane Herdr `w19:pV` |
| **Revisor adversarial** | Claude (Opus) | subagente em background |

Três famílias de modelo diferentes olhando o mesmo código — de propósito: cada
uma erra e enxerga diferente. O Grok, por rodar num pane do Herdr, validou coisas
que só quem *vive* dentro do Herdr consegue (ex.: `HERDR_PANE_ID` em
`/proc/<pid>/environ`, o needle com emoji no `wait-output`, o argv real do 0.8.0).

## O plano de controle

O orquestrador nunca editou código de produção. O trabalho dele foi:

1. **Despachar** — `herdr agent prompt <nome> "<tarefa>"` para cada caçador, em
   paralelo, cada um com uma **fatia disjunta de arquivos** (todos compartilham o
   mesmo working tree; fatias disjuntas eliminam conflito de edição).
2. **Esperar sem busy-wait** — os caçadores entregam relatórios como arquivos em
   `/tmp/herdr-pet-bug-hunt/REPORT-*.md` / `FIX*-*.md`; um monitor de filesystem
   acorda o orquestrador a cada arquivo que chega. Caçador travado é
   diagnosticado com `herdr agent read <nome> --source visible` e re-promptado.
3. **Conferir** — todo achado P0/P1 foi verificado contra o código (e contra o
   ambiente vivo: `herdr agent list`, `~/.grok/active_sessions.json`, o disco de
   `~/.claude*/projects/`) antes de entrar no veredito. Achismo foi descartado;
   duplicatas, fundidas.
4. **Commitar por autor** — os caçadores **não** commitam. O orquestrador roda a
   suíte inteira no tree integrado e faz um commit por caçador por rodada, com o
   crédito no corpo da mensagem.
5. **Fechar o loop adversarial** — depois de cada leva de fixes, o mesmo agente
   Opus (retomado com contexto acumulado) recebia o diff com a instrução de
   **refutar**. O que ele derrubava voltava pro caçador da fatia. Repetiu até o
   parecer vir limpo.

## As fases

| fase | rodadas | o que aconteceu |
|---|---|---|
| **Veredito** | 1 | 4 caçadores varrem em paralelo; orquestrador confirma/rejeita cada achado no código e escreve o veredito com prioridades |
| **P0** | 4 | 3 bugs que quebravam XP/anti-cheat; Opus refutou parte das 2 primeiras rodadas (ex.: replay de abertura no ordering dos trackers de save) e os furos voltaram pros caçadores |
| **P1** | 3 | 7 bugs de comportamento visível, em 2 ondas (fatias colidiam em `state.rs`); Opus achou 6 acionáveis, todos fechados |
| **P2/P3** | 3 | fantasmas por staleness, broken pipe, eviction, needle; validações ao vivo pelos próprios panes |

Do processo saíram também **2 bugs incidentais** que nenhuma varredura tinha
listado: o `pane wait-output` do farewell **nunca funcionou** (o herdr 0.8.0
exige o pane *antes* das flags — o erro era engolido e o toggle vivia só do
hold), e o retry de save martelava o disco a cada frame em fs read-only.

## Regras que fizeram funcionar

- **Fatias disjuntas, sempre.** Mesmo working tree + agentes paralelos só
  funciona se cada um tocar arquivos diferentes; quando duas tarefas colidiam
  num arquivo, viravam ondas sequenciais.
- **Relatório é arquivo, não chat.** Cada agente escreve um `.md` com diff,
  evidência e repro; o orquestrador (e o revisor) trabalham em cima do artefato,
  não de resumo de conversa.
- **Evidência ou nada.** O BRIEF exigia arquivo + trecho + repro; "sem teoria
  sem evidência". Vários suspeitos clássicos (SIGHUP sem save, idle gerando XP)
  foram *rejeitados* no veredito com prova.
- **Subcontar > inflar.** Toda ambiguidade de detecção resolve pra menos XP,
  nunca pra mais — a disciplina virou critério de design repetido em 4 fixes.
- **O revisor tem que ser adversarial e persistente.** O mesmo Opus, com o
  contexto das rodadas anteriores, achou furo real em **todas** as levas — 
  inclusive nos fixes que consertavam os furos que ele mesmo tinha achado.
- **Repro executada > repro descrita.** XP falso medido em segundos de relógio,
  state modo 000, dois watches concorrentes com KILL, EPIPE de verdade, A/B do
  argv contra o binário real.

## Artefatos

- Veredito e relatórios de cada rodada: `/tmp/herdr-pet-bug-hunt/` (efêmero;
  o que importa está nos commits).
- Histórico: `git log v0.3.1..fix/p0-bug-hunt` — cada commit nomeia o bug, a
  evidência e o caçador que consertou.
