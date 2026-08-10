# herdr-pet

Um companheiro de terminal que acompanha o programador no Herdr: reage ao status do agente e cresce com o trabalho real dele. Este é o glossário da linguagem do domínio — só conceitos, nada de implementação. Para design e instalação, veja [docs/design.md](docs/design.md).

## Language

### Identidade vs. jogo

**Pet**:
A identidade forjada e cosmética do companheiro — espécie, raridade, shiny, nome, proveniência. Derivada do GitHub ID do dono; ~imutável. Não carrega força de jogo.
_Avoid_: avatar, personagem

**Gameplay State**:
O estado de jogo mutável e persistido — XP, nível (derivado), HP/SP atuais. Vive separado do Pet.
_Avoid_: save, profile

**Força efetiva**:
A capacidade de combate do pet. Deriva do **nível** (trabalho), não dos genes.
_Avoid_: stats forjados, poder do gene

### Quem o pet observa

**Agente**:
O assistente de código (Claude Code, etc.) cujo trabalho o pet acompanha. Status: `working`, `done`, `blocked`, `idle`, `unknown`.
_Avoid_: modelo, LLM

### Progressão

**XP**:
O que o pet acumula a partir do trabalho real do agente. Tempo idle não rende XP.
_Avoid_: pontos

**Nível**:
O posto do pet, de 1 a 99. Derivado do XP total por uma curva — não é guardado à parte.
_Avoid_: rank

**Trabalho acompanhado**:
Período em que o pane do pet está aberto e o agente está `working`. Rende XP no ritmo cheio.
_Avoid_: farm ativo, grind

**Trabalho não acompanhado**:
Trabalho do agente que rolou com o pane fechado. Contabilizado na reabertura do pet, num ritmo menor que o acompanhado.
_Avoid_: XP passivo, idle XP

**Sinal de trabalho**:
O indicador, lido do Herdr, de que o agente de fato trabalhou: o status `working` e o contador `state_change_seq`. Não editável localmente — base do anti-cheat.
_Avoid_: contador de tempo, heartbeat

**Rebirth**:
(Futuro) reset no nível 99, com marcador cosmético e bônus. Fora do escopo atual.
