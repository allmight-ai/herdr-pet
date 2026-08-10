# XP agrega todos os agentes, com decaimento harmônico de largura

**Status:** accepted

O XP conta o trabalho de **todos** os agentes detectados (qualquer projeto), não só o focado. Vários agentes trabalhando ao mesmo tempo não somam linearmente: cada agente extra rende uma fração menor (1, ½, ⅓, … — série harmônica), então proliferar agentes pra multiplicar XP dá retornos decrescentes (8 agentes ≈ 2,7×, não 8×).

Agregamos todos (e não só o focado) porque o pet deve crescer com tudo que o programador faz, em qualquer projeto. O decaimento harmônico porque linear inflacionaria (e abriria brecha de proliferação de agentes); um cap duro tem cliff arbitrário; a harmônica é suave e mantém o caso comum — 1 agente — no ritmo cheio, sem penalidade.

**Consequências:** o display (humor/tarefa) continua espelhando o agente **focado**; só o XP agrega todos. A curva é a defesa contra inflação por proliferação de agentes. Ver também `0001-xp-from-real-agent-work.md`.
