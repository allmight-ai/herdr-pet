#!/usr/bin/env bash
# Cria um release completo: bump de versão + CHANGELOG + commit + tag + push.
# A GitHub Action (.github/workflows/release.yml) publica a Release no GitHub ao
# receber a tag — este script só cuida do lado local.
#
# Uso:  ./scripts/release.sh <X.Y.Z>     ex.: ./scripts/release.sh 0.3.0
#
# Pre-reqs: working tree limpa, na branch que você publica (main), permissão de push.
set -euo pipefail

VERSION="${1:?uso: release.sh <X.Y.Z> (ex.: 0.3.0)}"
TAG="v${VERSION}"
cd "$(git rev-parse --show-toplevel)"

# 1) working tree limpa
if ! git diff --quiet --cached || ! git diff --quiet; then
  echo "✗ working tree suja. Commit ou stash antes de releasear." >&2
  exit 1
fi

CUR="$(grep -m1 '^version' herdr-plugin.toml | sed 's/.*"\(.*\)".*/\1/')"
echo "→ ${CUR}  ⇒  ${VERSION}  (${TAG})"

# 2) bump de versão nos dois .toml (-i.bak = portátil entre Linux e macOS)
sed -i.bak "s/^version = \"[^\"]*\"/version = \"${VERSION}\"/" Cargo.toml herdr-plugin.toml
rm -f Cargo.toml.bak herdr-plugin.toml.bak

# 3) recompila: valida que builda e mantém Cargo.lock sincronizado (se houver)
cargo build --release --quiet

# 4) seção do CHANGELOG com os commits desde a última tag
LAST_TAG="$(git describe --tags --abbrev=0 2>/dev/null || true)"
DATE="$(date +%Y-%m-%d)"
SECTION="$(mktemp)"
{
  echo "## [${VERSION}] - ${DATE}"
  echo
  if [ -n "$LAST_TAG" ]; then
    echo "Mudanças desde \`${LAST_TAG}\`:"
    echo
    git log --format='- %s' "${LAST_TAG}..HEAD"
  else
    echo "- Release inicial."
  fi
  echo
} > "$SECTION"

# insere a seção antes da primeira "## [" existente (ou no topo, se não houver)
if grep -q '^## \[' CHANGELOG.md; then
  awk -v f="$SECTION" '/^## \[/ && !d {while((getline l < f)>0) print l; d=1} {print}' \
    CHANGELOG.md > CHANGELOG.md.new && mv CHANGELOG.md.new CHANGELOG.md
else
  cat "$SECTION" CHANGELOG.md > CHANGELOG.md.new && mv CHANGELOG.md.new CHANGELOG.md
fi
rm -f "$SECTION"

# 5) commit + tag + push (branch atual + tag)
git add Cargo.toml herdr-plugin.toml CHANGELOG.md
[ -f Cargo.lock ] && git add Cargo.lock
git commit -m "release: ${TAG}" -q
git tag "$TAG"
git push origin HEAD "$TAG"

echo "✓ ${TAG} criado e pushado. A Action publica a Release no GitHub."
