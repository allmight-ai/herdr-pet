# Mantendo o projeto: documentação e releases

Referência pra duas coisas que se repetem neste projeto: **(1) atualizar a
documentação** quando o comportamento muda, e **(2) cortar uma nova release**.
Salvo aqui pra não ter que lembrar de cabeça.

## 1. Atualizar a documentação

Sempre que mexer no comportamento visível do pet, atualize a doc junto — senão ela
passa a mentir. Mapa do que tocar por tipo de mudança:

| Mudou o quê... | Atualizar aonde |
| --- | --- |
| Status / humor / animação / display do pet | `README.md` (PT **e** EN, seção "Como funciona"), `docs/design.md` |
| XP / nível / curva / ritmo | `README.md`, `docs/design.md`, `docs/adr/` |
| Termo ou conceito do domínio | `CONTEXT.md` (o glossário) |
| Decisão de arquitetura nova ou revertida | novo ADR `docs/adr/NNNN-*.md` (próximo número livre) |
| Processo de build / release / dev hooks | este arquivo (`docs/release.md`) e `README.md` ("Desenvolvimento local") |

E **sempre** que cortar uma release (próxima seção), adicione uma entrada no
`CHANGELOG.md`.

> Regra de ouro: a doc não é decoração — se você mudou o que o pet faz ou como ele
> ganha XP, alguém lendo o README tem que combinar com o que o binário faz.

## 2. Cortar uma release

Uma **release** é uma versão numerada (`vX.Y.Z`) publicada no GitHub, com notas do
que mudou desde a anterior. Ela sai de um commit do `main` e é marcada com uma tag.

### Pré-requisito (uma vez na vida): auth com scope `workflow`

O push de uma release carrega o `.github/workflows/release.yml`, e o GitHub **exige**
um token com permissão `workflow` pra criar/atualizar arquivos de CI. O login OAuth
padrão do `gh` **não** traz esse scope, e em alguns setups o `gh auth refresh` trava
em keyring. A saída robusta é um **Personal Access Token (PAT)** no credential store
do git — configurado uma vez, vale pra todos os releases (e pro script).

1. Crie um PAT **classic** em: GitHub → Settings → Developer settings →
   Personal access tokens → **Tokens (classic)** → Generate new token.
   Marque os scopes **`repo`** e **`workflow`**. Copie o token.
2. Configure o git pra usá-lo (substitui o helper do `gh`, que pode estar quebrado):

   ```bash
   git config --global credential.https://github.com.helper ""
   git config --global credential.https://github.com.helper store
   printf 'https://x-access-token:COLE_O_TOKEN_AQUI@github.com\n' > ~/.git-credentials
   chmod 600 ~/.git-credentials
   ```

   A partir daqui, qualquer `git push` (inclusive o do script abaixo) autentica
   sozinho com o PAT. O token fica só na sua máquina, em `~/.git-credentials` (modo `600`).

### O release em si — um comando

```bash
./scripts/release.sh 0.3.0
```

O `scripts/release.sh`:

- exige working tree limpa (commit/stash antes);
- sobe a versão em `Cargo.toml` **e** `herdr-plugin.toml` (precisam bater);
- adiciona uma seção no topo do `CHANGELOG.md` com os commits desde a última tag;
- commita `release: vX.Y.Z`, cria a tag `vX.Y.Z` e faz push (branch + tag).

No push da tag, a GitHub Action (`.github/workflows/release.yml`) publica a Release
no GitHub **sozinha**, com as notas extraídas do `CHANGELOG.md`. Se a seção não
existir, ela cai pras notas automáticas do GitHub.

> **Versionamento (SemVer, em `0.x`):** suba o **MINOR** (`0.2.0 → 0.3.0`) quando
> adicionar feature; o **PATCH** (`0.2.0 → 0.2.1`) pra só correção de bug. Antes do
> `1.0.0` a API pode mudar livremente — o MINOR é o sinal de "tem coisa nova".

## 3. Rebuild automático durante o desenvolvimento

Os git hooks em `githooks/` (`post-commit`, `post-merge`), ativados via
`git config core.hooksPath githooks`, recompilam o pet (`cargo build --release`)
após cada commit/pull. Assim o binário que o Herdr executa fica sempre sincronizado
com o código — sem precisar lembrar de rebuildar à mão (era a causa do bug "pet só
ganha XP da sessão ativa": binário desatualizado).

Num clone novo, ative com:

```bash
git config core.hooksPath githooks
```

## Resumo do fluxo de cada release

1. Desenvolve no `main` (a doc vai sendo atualizada junto — seção 1).
2. Confirme que `git status` está limpo e os testes passam (`cargo test`).
3. `./scripts/release.sh X.Y.Z` → bump + CHANGELOG + commit + tag + push.
4. A GitHub Action publica a Release com as notas.
5. Reabra o pane do pet no Herdr pra ele pegar o binário novo.
