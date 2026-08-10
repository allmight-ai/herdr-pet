# XP vem do trabalho real do agente, não do tempo

**Status:** accepted

O pet ganha XP a partir do trabalho **real** do agente — o status `working` ao vivo e o contador `state_change_seq` do Herdr como prova de atividade — nunca de tempo idle. Com o pane aberto e o agente trabalhando, o XP flui no ritmo cheio (*trabalho acompanhado*); com o pane fechado, o trabalho que rolou é contabilizado na reabertura a partir do delta de `state_change_seq`, num ritmo menor (*trabalho não acompanhado*). O nível (1–99) é **derivado** do XP total por uma curva, não guardado à parte.

Escolhemos assim porque o sinal de trabalho vem do Herdr (não de um arquivo local editável) e idle não rende nada — então não dá pra farmar só deixando rodando.

**Alternativas rejeitadas:** basear XP em tempo decorrido (idle farmável, fácil de burlar); manter um daemon sempre ligado (quebra o modelo on-demand do Herdr, que não tem pane global); guardar o nível em vez de derivá-lo (mais estado e sincronização pra manter, sem ganho no MVP).

**Consequências:** o nível recalcula automaticamente se a curva mudar — aceitável num jogo local single-player. A granularidade exata de `state_change_seq` por unidade de trabalho é incerta e exige calibração empírica; até lá, os números de ritmo são ajustáveis sem mudar o design.
