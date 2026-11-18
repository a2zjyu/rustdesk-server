#!/usr/bin/env bash
# 레포의 현재 커밋으로 이미지를 빌드한다. 실행 중인 hbbs/hbbr 에는 영향 없음 (sudo 불필요)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
[[ -z "$(git status --porcelain)" ]] || { echo "커밋 안 된 변경이 있습니다" >&2; exit 1; }
# 하위 모듈이 비어 있거나(-) 기록된 커밋과 다르면(+) 거부
if git submodule status | grep -q '^[-+U]'; then
  echo "하위 모듈이 준비되지 않았습니다: git submodule update --init" >&2
  exit 1
fi
REV=$(git rev-parse --short=12 HEAD)
TAG="a2z/rustdesk-server:$REV"
echo "빌드: $TAG ($(git log --oneline -1))"
DOCKER_BUILDKIT=1 docker build --progress=plain \
  --label org.opencontainers.image.source=https://github.com/a2zjyu/rustdesk-server \
  --label org.opencontainers.image.revision="$(git rev-parse HEAD)" \
  -f deploy/Dockerfile -t "$TAG" .
echo "완료: $TAG"
docker image inspect "$TAG" --format '  이미지 ID: {{.Id}}  크기: {{.Size}}'
