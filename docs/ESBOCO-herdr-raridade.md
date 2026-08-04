# Esboço — Herdr Pet: raridade forjada, não sorteada

**Status:** DESIGN / esboço, **não publicado, não implementado**. Continuação do raciocínio do
[`ECOSSISTEMA-gacha-troca.md`](./ECOSSISTEMA-gacha-troca.md) (proveniência) e da virada V-Pet LCD
de [`SESSAO-vpet-2026-07-11.md`](./SESSAO-vpet-2026-07-11.md). Alvo: companion (plugin) que vive
numa pane do **Herdr**, reaproveitando a teoria do petterm.

## A virada de modelo (1 frase)

Hoje no petterm a raridade nasce de `secrets.SystemRandom()` (em `core.py:roll_birth`) e fica
selada num HMAC local cuja chave é `machine.id` → **dono da máquina pode apagar `~/.petterm` e
re-rolar**. No companion a raridade vai **derivar deterministicamente de uma âncora externa
imutável** → apagar o state e re-rodar **re-deriva o mesmo pet**. Não há reroll.

É o mesmo insight já mapeado em `ECOSSISTEMA-gacha-troca.md` ("valor exige custódia server-side"),
só que **sem servidor** a melhor custódia grátis e verificável é a **identidade GitHub**.

## O mecanismo: âncora = ID numérico do GitHub

`gh api user --jq .id` devolve o ID numérico da conta — **imutável, atribuído pelo GitHub, o dono
não controla**. Ele vira a semente:

```python
def hatch_from_anchor(github_numeric_id: int):
    seed = hmac.new(APP_SALT, str(github_numeric_id).encode(), hashlib.sha256).digest()
    rng = DeterministicRng(seed)      # DRBG, NÃO secrets.SystemRandom
    return roll_birth(rng)            # mesmos pesos 60/25/10/4/1, shiny 1/128
```

Só isso já fecha o reset: `load_or_hatch` deixa de chamar `roll_birth()` aleatório e vira
**idempotente** — deleta tudo, roda de novo, **nasce o mesmo pet**, porque a raridade nunca esteve
no disco.

**Por que é mais forte que o HMAC atual:** o HMAC do petterm move a confiança pra `machine.id`
(UUID que o dono forja/recria). Aqui a âncora é **fora da máquina**. Anti-cheat estrito sem
servidor.

## DNA: genes por sub-seed (reserva ilimitada)

A semente vira genes por **derivação por nome** (sub-seeds), não por slots fixos num fluxo contínuo.
Cada gene é uma função pura de `(seed, nome_da_feature)`:

```python
def gene(seed: bytes, name: str) -> bytes:
    return hmac.new(seed, name.encode(), hashlib.sha256).digest()   # 32 bytes

species = map_species(gene(seed, b"species"))   # hoje
rarity  = map_rarity(gene(seed, b"rarity"))     # hoje
shiny   = bool(gene(seed, b"shiny")[0] & 1)     # hoje
# "reservado" = só batizar um nome novo quando precisar:
# iv               = gene(seed, b"iv")
# nature           = gene(seed, b"nature")
# battle_tendency  = gene(seed, b"battle.tendency")
```

Mesmo princípio da derivação hierárquica do BIP-32 (carteiras HD): **adicionar um gene novo nunca
muda um gene existente**, porque cada um depende só da semente + do seu próprio nome. É o formato
escolhido em vez de slots fixos reservados (que exigem adivinhar o futuro e podem esgotar/colar).

Blindagem do nascimento contra crescimento futuro:
- **Nunca esgota** — feature nova é um nome novo, sem reservar espaço físico.
- **Nunca cola nem reordena** — mover/adicionar/remover um gene não altera os outros.
- **Pet já nascido não muda** — o gene "já existia" matematicamente no nascimento, mesmo antes de
  ser lido. A idempotência se mantém. ✅

`APP_SALT` (segredo do app, embarcado no código) e o `name` de cada feature são os dois contratos
estáveis. Mudar qualquer um dos dois é bump de versão (ver a seguir).

## Versionamento & alterabilidade: o que dá pra mudar depois

**Regra de ouro: o algoritmo é mutável; os nascimentos são imutáveis.** Cada pet grava o
`genesis_version` com que nasceu; a re-derivação respeita essa versão (ou roda uma *migration*
explícita e versionada). Novas versões do plugin não reescrevem a história.

Proveniência ganha dois campos:

```json
"provenance": {
  "origin": "herdr-companion-v1",
  "genesis_version": 1,
  "anchor_kind": "github_user_id",
  "derivation": "hmac-sha256-subseed",
  "seed_hash": "…",
  "server_sig": null
}
```

O que é possível alterar depois, por categoria:

| Quero… | Dá? | Como |
|---|---|---|
| **Adicionar gene novo** (IV, natureza, batalha) | ✅ Sempre, grátis | Batiza um nome novo de sub-seed. Pets antigos também ganham (o gene já existia). Sem bump de versão. |
| **Mudar schema/estrutura** (renomear campo, trocar encoding) | ✅ Sim | `genesis_version` + migration. Não afeta raridade/espécie → não afeta valor. |
| **Mudar a *leitura* de um gene** (ex.: shiny agora usa 2 bytes em vez de 1) | ⚠️ Sim, com bump | Nova versão do mapeamento; pets antigos congelam na leitura antiga. |
| **Mudar `APP_SALT` ou a derivação** | ⚠️ Sim, disruptivo | Tudo re-deriva diferente; bump global. Pets existentes ficam na versão velha. |
| **Mudar raridade/espécie de um pet já nascido** | ❌ Não se faz | Tecnicamente dá via migration, mas **quebra a promessa de forja** e contradiz o gist público já timestampado. O correto é versão nova só pra pets *novos*; velhos continuam velhos. |

Resumo: **adicionar é livre; mudar estrutura é versionado e tranquilo; mudar a *identidade* de um
pet já nascido é a única coisa deliberadamente proibida** — porque é essa imutabilidade que dá valor
à raridade.

## Separação limpa: identidade vs. condição

| Camada | Origem | Resetável? |
|---|---|---|
| **Identidade** (espécie, raridade, shiny, gene) | derivada da âncora GitHub | **Não** — idempotente |
| **Condição** (fome, humor, energia, saúde) | local, HMAC-assinado como hoje (`core.py`) | Sim (gameplay), com anti-clock-cheat |

Só a parte que importa pra "valor" (identidade) vira não-resetável. Os stats seguem locais e
decaindo no tempo real.

## Proveniência (reusa o modelo do petterm, só muda a âncora)

```json
{
  "id": "uuid-v4",
  "species_id": "griffin",
  "rarity": "epic",
  "shiny": false,
  "anchor": "github:12345678",
  "born_at": "2026-08-03T…Z",
  "provenance": {
    "origin": "herdr-companion-v1",
    "anchor_kind": "github_user_id",
    "derivation": "hmac-sha256",
    "seed_hash": "…",
    "server_sig": null
  }
}
```

`server_sig = null` hoje (pet local não-verificado, mas jogável pra sempre — open source first).
Quando existir servidor, rolls oficiais ganham `roll_seed_hash` + assinatura → só assinados entram
em troca/mercado. **Custa zero agora, evita migração depois.**

## Trava anti-reroll (porque "criar conta nova" ainda é um reroll)

1. **Lock-in**: na 1ª eclosão, grava o anchor visto. Daí pra frente, trocar de conta GitHub **não**
   rerolla — fica travado na primeira.
2. **Genesis gist**: nessa 1ª eclosão, posta um gist público `{anchor, seed_hash, born_at}`
   (commit-reveal). A partir dali a raridade é **timestamped e pública** — qualquer tentativa de
   reroll seria visível (novo gist = "tentou de novo"). Não é prova cripto perfeita sem servidor,
   mas é transparência: a comunidade vê.

## Camada cultivada: raridade que cresce por mérito

O tier base é **forjado** (fixo, derivado da âncora). Por cima, uma **aura** cresce com trabalho
verificável — os mesmos eventos que alimentam o pet (PR mergeado, issue fechada, CI verde,
contribuição contínua via `gh api`). Esses sinais **acumulam** e não dá pra resetar sem apagar o
histórico público real. A aura desbloqueia variantes (shiny "por mérito", moldura de LCD, título,
evolução visual) **sem mexer no tier forjado**. O bicho amadurece com a carreira — e os mesmos
eventos que enchem a barriga forjam a aura.

## Mapeamento no Herdr (doc oficial confirmada — v0.8.0)

Plugin nativo do Herdr = **manifest `herdr-plugin.toml`** + **binário Rust**. O Herdr executa o
`command` (argv array) de cada entrada — linguagem livre (doc: *"a Bash script, JavaScript app, Lua
script, **Rust binary**, or any other argv command"*). Rendering de pane = **stdout/ANSI**
(documentado; **não há protocolo de gráficos** — LCD 1-bit em Unicode block chars + ANSI é o caminho
oficial). Doc: https://herdr.dev/docs/plugins/

Schema confirmado (seções `[[...]]`): `[[build]]`, `[[startup]]`, `[[actions]]`, `[[events]]`,
`[[panes]]`, `[[link_handlers]]`, `[[keys.command]]`. **Não existem**: `[[commands]]`,
`[[settings]]`/`[[config]]` (config do usuário = arquivo em `HERDR_PLUGIN_CONFIG_DIR`).

**Estrutura alvo do manifest do companion:**

```toml
id = "fredericotmello.herdr-pet"          # + name, version, min_herdr_version, platforms
[[build]]
command = ["cargo", "build", "--release"]  # Herdr compila o Rust no install (do GitHub)
[[startup]]
command = ["./target/release/herdr-pet", "startup"]   # boot: hatch/lock-in inicial + sync de aura
[[panes]]
id = "lcd"
title = "Pet"
placement = "split"
command = ["./target/release/herdr-pet", "watch"]     # casinha LCD no stdout
[[actions]]
id = "feed"   # ... play / sleep / status / reborn  (comandos do joguinho)
[[events]]
on = "worktree.created"                     # acordar/celebrar + re-sync de aura
command = ["./target/release/herdr-pet", "on-event"]
[[link_handlers]]
id = "github-pr"                            # petisco: clicar em PR/issue alimenta
pattern = "^https://github\\.com/.+/(pull|issues)/[0-9]+$"
action = "feed"
```

- **`[[build]]`** = `cargo build --release` → Herdr compila no `install`. Sem runtime.
- **`[[startup]]`** roda 1× após restaurar sessão: hatch/lock-in inicial + sync de aura.
- **`[[panes]]` `placement=split`** = casinha LCD persistente no stdout (cor = raridade).
- **`[[events]]`** confirmado: `on` validado no link time (worktree.created, pane.focused, …). Acorda
  o pet e dispara re-sync de aura **sem polling cego** — mas a aura em si (PR mergeado, issue fechada)
  vem do GitHub via `gh api`, então `startup` + eventos disparam essas leituras.
- **`[[link_handlers]]`** `pattern` é **regex Rust** → petiscos ao clicar em PR/issue.
- State em `HERDR_PLUGIN_STATE_DIR` (não em `HERDR_PLUGIN_ROOT`). **Raridade nunca vive só lá** —
  sempre re-derivável da âncora.

Env vars de runtime (doc oficial): `HERDR_SOCKET_PATH`, `HERDR_BIN_PATH` (CLI `herdr`),
`HERDR_PLUGIN_ID`, `HERDR_PLUGIN_ROOT`, `HERDR_PLUGIN_CONFIG_DIR`, `HERDR_PLUGIN_STATE_DIR`,
`HERDR_PLUGIN_CONTEXT_JSON` (workspace/tab/pane/worktree/agent/selected/clicked-url), `HERDR_PANE_ID`,
`HERDR_PLUGIN_EVENT(_JSON)`, etc. **Sem SDK oficial** — a "API do plugin" é a CLI `herdr` (subprocesso)
ou JSON over `HERDR_SOCKET_PATH`. (`HERDR_CELL_*_PX` só o plugin `browser` usa — não documentadas; LCD
em células de caractere não precisa.)

## O limite honesto

Sem servidor, "não dá pra resetar" = **"não dá sem trocar de identidade GitHub"** (e com lock-in +
gist, nem isso é silencioso). Exclusividade real entre colegas/time só com server. Esse é o teto.

---

## DECISÃO FECHADA (2026-08-04): lock-in na 1ª conta

**Escolha: opção (A).** O pet fica vinculado à **primeira conta GitHub** vista na eclosão. Trocar
de conta **não** rerolla. Genesis gist público (commit-reveal) mantido.

O caso legítimo de multi-conta (pessoal + trabalho) é uma **limitação assumida, não um bug** —
comunicada com transparência no README em vez de escondida. Esse é o preço de uma raridade que não
dá pra resetar sem servidor.

---

## Rascunho — seção do README (explicação pro usuário final)

> ### Seu pet é forjado pela sua conta GitHub
>
> A raridade do seu pet **não é sorteada** — ela é *forjada* deterministicamente a partir do **ID
> numérico da sua conta GitHub** (um número que o GitHub atribui e que você não controla). Por isso:
>
> - **Não dá pra resetar a raridade.** Apagar os arquivos do plugin e rodar de novo *nasce o mesmo
>   pet*. A raridade nunca esteve no seu disco — ela é re-derivável da âncora.
> - **Não dá pra rerollar trocando de conta.** O pet fica vinculado à **primeira conta GitHub** que
>   você usou. Isso é de propósito: é o que torna a raridade *real*. Se bastasse trocar de conta
>   pra tirar um lendário, ela não valeria nada.
>
> #### E se eu tiver mais de uma conta GitHub?
>
> É comum (pessoal + trabalho, por exemplo). O pet vai pertencer à **primeira conta** usada na
> eclosão. Essa é uma escolha consciente de design: preferimos uma raridade que não dá pra resetar
> a uma raridade "conveniente". Fazer o pet responder a outra conta exigiria um servidor de
> custódia — fora do escopo deste companion open source.
>
> #### Transparência pública
>
> Na primeira eclosão o plugin posta um **gist público** com `{anchor, seed_hash, born_at}`. Isso
> *timestampa* o nascimento do seu pet: qualquer tentativa de "começar de novo" seria visível (um
> segundo gist = "tentou de novo"). Sem servidor não é prova cripto perfeita, mas é transparência —
> a comunidade vê.
>
> #### O limite honesto
>
> Sem servidor, "não dá pra resetar" significa "não dá **sem trocar de identidade GitHub**, e com o
> lock-in nem isso é silencioso". Exclusividade real (só um de cada por pessoa, mercado de troca
> confiável) só com servidor. Esse é o teto de um companion 100% local.

---

## DECISÃO (2026-08-04): Virada pra Visão B — coleção forjada por mérito

Mudança de filosofia: o companion deixa de ser "1 pet = sua identidade" (Visão A) e passa a ser
**"sua coleção = sua carreira"** (Visão B) — jogo de coleção/farm inspirado em Pokémon (buscar o
melhor, IV), mas **sem reroll**.

**Motivo:** com 1 pet forjado imutável, ~60% do público nasce em common e fica preso (sem farm, sem
saída) → fracasso pra maioria. Em Pokémon isso não dói porque se farma outro. A saída compatível
com a tese é a **coleção forjada por mérito**:

- A âncora GitHub deriva uma **coleção** (vários pets), não um único. Cada pet é forjado por
  `(âncora, índice)`.
- O usuário **não nasce** com todos: cada nova eclosão **custa aura** (mérito verificável — PR,
  issue, CI verde via `gh api`). Custo irreversível e público.
- O **índice de pets é público** (linha do tempo no gist). Não dá pra pular índices sem pagar; não
  dá pra savescum (o tier de cada índice é fixo desde a forja). → anti-reroll **mantido**.

**Unificação arquitetural:** renascimento (R2) e coleção são o **mesmo mecanismo** — "eclosão por
índice":

```python
gene(seed, b"pet:0")   # nascimento
gene(seed, b"pet:1")   # 1ª eclosão custosa (aura) — renascimento (R2) se substitui o ativo
gene(seed, b"pet:2")   # …se junta à coleção = farm (toggle futuro, quase grátis por já ser índice)
```

Implementar R2 agora nesse formato **deixa a coleção como toggle futuro quase gratuito**.

**Decisões firmadas neste pacote:**
- Espécies: **pixel-mons inventados** (direção b) — originais, formas fáceis em 1-bit LCD. Catálogo
  a criar.
- Renascimento: **R2 forjado** (eclosão por índice; custo de aura entra quando a aura existir).
- Curva de raridade: **mantida 60/25/10/4/1** (não suavizar — manter a dificuldade).
- Aura ganha peso central: vira a **moeda da eclosão**, não só cosmética.

> TODO: o rascunho do README acima (Visão A, "1 pet") precisa ser reescrito pra Visão B quando o
> design estabilizar. Anti-reroll / lock-in / gênese por âncora seguem válidos.

---

## Futuro: Breeding estilo Pokémon (adaptado ao determinismo)

**Diretriz (2026-08-04):** o breeding seguirá as **regras de Pokémon** (comunidade competitive
breeding é grande e as regras são consagradas), porém **adaptado ao nosso modelo determinístico e
não-resetável**. É **endgame** — só depois do farm/aura (Fase 3) e da coleção (Fase 5+). NÃO
implementar agora.

### Regras de Pokémon a portar
- **6 IVs** (HP, Atk, Def, Sp.Atk, Sp.Def, Speed), 0–31 cada.
- **25 Natures**: +10% num stat, -10% em outro.
- **Herança de IV**: ~3 de 12 vêm dos pais (Destiny Knot = 5 herdados).
- **Nature herdável** (Everstone = 100% de um pai).
- **Masuda Method**: pais "diferentes" aumentam chance de shiny.

### A adaptação CRÍTICA (sem virar reroll)
Pokémon breeding é **RNG** (centenas de ovos rerollando). O nosso é **determinístico + custoso**: o
filho é forjado por `(hash dos pais, breed_counter)` e **custa aura** (e/ou consome um pai), de
forma **irreversível**. Não há reroll de ovos — a "busca" é **estratégica** (quais pais cruzar,
gestão de aura/contador), não grind. Preserva a tese (não-resetável) ao custo de não ter o farm de
ovos do Pokémon. Trade-off consciente.

### Modelo recomendado (híbrido)
- Filho de **tier superior** (Mythic/Divine — só acessível via breeding, jamais na forja normal) +
  herda **IV/nature dos pais** (Pokémon-style).
- `child_seed = gene(combine(parent_a.seed, parent_b.seed, breed_counter), "breed")`
- Combina o endgame de tier superior (estilo idle) com a profundidade genética (Pokémon).

### Decisões em aberto (quando retomar)
- **Puro-Pokémon** (mesma espécie, só aperfeiçoa IV) vs **sobe-tier** (Mythic+) → proposto: híbrido.
- Quantos IVs herdam por padrão; se há "itens" consumíveis (Destiny Knot / Everstone) via aura.
- Egg groups (faz sentido com poucas espécies?) ou "qualquer um cruza com qualquer um".
