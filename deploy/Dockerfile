# 사내 RustDesk 서버(포크) 이미지 빌드 - 레지스트리 없이 서버에서 직접 빌드
#   소스: 이 레포 (https://github.com/a2zjyu/rustdesk-server), 빌드 컨텍스트는 레포 루트
#   결과: FROM scratch 에 정적 링크된 hbbs/hbbr 만 담음 (공식 classic 이미지와 같은 구조)
#   빌드: deploy/build.sh  (태그 a2z/rustdesk-server:<커밋해시>)
#   보낼 파일 목록: deploy/Dockerfile.dockerignore (BuildKit 이 Dockerfile 옆의 것을 읽음)

# rust:alpine (rustc 1.100.0, Alpine 3.24) - musl 정적 링크
FROM rust:alpine@sha256:8d0e82abfdf9a38071e07cabc5afba4ddbe077631a80e4e742a760c38beadc33 AS build
# openssl-sys(의존성)가 빌드 중 OpenSSL을 찾으므로 정적 라이브러리까지 넣는다
RUN apk add --no-cache musl-dev build-base perl cmake pkgconf openssl-dev openssl-libs-static
ENV OPENSSL_STATIC=1
WORKDIR /src
COPY . ./
# sqlx 쿼리 매크로는 .env 의 DATABASE_URL 로 db_v2.sqlite3 를 읽어 스키마를 확인한다
# target/ 캐시: build.rs 가 src/version.rs 를 감시하므로 새 소스에서는 build.rs 가 다시 돈다
# 테스트는 dev 프로필로 --lib --tests 를 돌린다 (단위 테스트 + tests/ 통합 테스트.
#   --bin hbbs 로 돌리면 0개가 실행되고, --lib 만 쓰면 tests/protobuf_recursion.rs 가 빠짐)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target,sharing=locked \
    cargo test --locked --lib --tests \
 && cargo build --release --locked --bin hbbs --bin hbbr \
 && mkdir /out && cp target/release/hbbs target/release/hbbr /out/

FROM scratch
COPY --from=build /out/hbbs /out/hbbr /usr/bin/
WORKDIR /root
ENV HOME=/root
