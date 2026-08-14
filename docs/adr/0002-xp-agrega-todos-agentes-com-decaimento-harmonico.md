# XP agrega todos os agentes, com decaimento harmônico de largura

**Status:** accepted

O XP conta o trabalho de **todos** os agentes detectados (qualquer projeto), não só o focado. Vários agentes trabalhando ao mesmo tempo não somam linearmente: cada agente extra rende uma fração menor (1, ½, ⅓, … — série harmônica), então proliferar agentes pra multiplicar XP dá retornos decrescentes (8 agentes ≈ 2,7×, não 8×).

No catch-up o peso é **por contribuinte**, ganhos em ordem decrescente, **janela top-8** (`g₀×1 + … + g₇×⅛`; do 9º em diante 0). Assim catch-up e live saturam iguais: 8 agentes ≈ 2,7×, nunca 3,6× com n=20. Quando os ganhos são iguais isso coincide com `g·H(min(n,8))`; quando não são, um tick minúsculo no top-8 só paga o próprio 1/k e não taxa o worker. O live continua com `H(n)` (teto 8) sobre o tempo.

Agregamos todos (e não só o focado) porque o pet deve crescer com tudo que o programador faz, em qualquer projeto. O decaimento harmônico porque linear inflacionaria (e abriria brecha de proliferação de agentes); um cap duro tem cliff arbitrário; a harmônica é suave e mantém o caso comum — 1 agente — no ritmo cheio, sem penalidade.

**Consequências:** só o XP agrega todos (com decaimento harmônico). *(Atualizado em 2026-08-12: o display agora agrega também — se qualquer agente está `working`, o pet acorda com a tarefa dele; só quando ninguém trabalha é que espelha o agente focado. Antes o display espelhava só o focado; ver `agent::aggregate_display`.)* A curva é a defesa contra inflação por proliferação de agentes. Ver também `0001-xp-from-real-agent-work.md`.
