# 사내 RustDesk 서버(포크) 이미지 빌드 - 레지스트리 없이 서버에서 직접 빌드
#   소스: 이 레포 (https://github.com/a2zjyu/rustdesk-server), 빌드 컨텍스트는 레포 루트
#   결과: FROM scratch 에 정적 링크된 hbbs/hbbr 만 담음 (공식 classic 이미지와 같은 구조)
#   빌드: deploy/build.sh  (태그 a2z/rustdesk-server:<커밋해시>)
#   제외 목록: deploy/Dockerfile.dockerignore (BuildKit 이 Dockerfile 옆의 것을 읽음)

# rust:alpine (rustc 1.100.0, Alpine 3.24) - musl 정적 링크
FROM rust:alpine@sha256:8d0e82abfdf9a38071e07cabc5afba4ddbe077631a80e4e742a760c38beadc33 AS build
# openssl-sys(의존성)가 빌드 중 OpenSSL을 찾으므로 정적 라이브러리까지 넣는다
RUN apk add --no-cache musl-dev build-base perl cmake pkgconf openssl-dev openssl-libs-static
ENV OPENSSL_STATIC=1
WORKDIR /src
COPY . ./
# sqlx 쿼리 매크로가 빌드 중에 이 DB 파일로 스키마를 확인한다 (.env 와 같은 값)
ENV DATABASE_URL=sqlite://./db_v2.sqlite3
# target/ 은 캐시하지 않는다: build.rs 가 src/version.rs 를 소스 폴더에 만드는데,
# target 캐시가 남아 있으면 build.rs 가 다시 돌지 않아 그 파일이 빠진다
# 테스트는 --lib 로 돌린다 (--bin hbbs 로 돌리면 테스트가 0개 실행됨)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo test --release --locked --lib \
 && cargo build --release --locked --bin hbbs --bin hbbr \
 && mkdir /out && cp target/release/hbbs target/release/hbbr /out/

FROM scratch
COPY --from=build /out/hbbs /out/hbbr /usr/bin/
WORKDIR /root
ENV HOME=/root
