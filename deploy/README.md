# 사내 배포용 이미지 빌드

레지스트리 없이 서버에서 직접 이미지를 빌드한다.

```bash
git clone --recurse-submodules https://github.com/a2zjyu/rustdesk-server
cd rustdesk-server
deploy/build.sh        # → a2z/rustdesk-server:<커밋12자리>
```

- 커밋 안 된 변경이 있거나 하위 모듈(`libs/hbb_common`)이 준비되지 않았으면 빌드를 거부한다.
- 실행 이미지는 `FROM scratch`에 `hbbs`, `hbbr`만 담는다. 공식 classic 이미지와 같은 구조라 기존 compose 설정(비root 1001, 읽기 전용, `./data:/root`)을 그대로 쓴다.
- 빌드 단계의 `target/`은 캐시하지 않는다. 캐시가 남으면 `build.rs`가 다시 돌지 않아 `src/version.rs`가 빠지고 컴파일이 실패한다.
- 테스트는 `cargo test --lib`으로 돈다. `--bin hbbs`로 돌리면 0개가 실행된다.

## 배포

`docker-compose.yml`에서 `image` 한 줄만 바꾼다. 공식 digest는 되돌리기용으로 주석에 남긴다.

```yaml
  # image: rustdesk/rustdesk-server@sha256:8ecdab65deb7c84652a626380e31d11a8f1fbafd97916d57f95c20628f943c00  # 공식 1.1.16
  image: a2z/rustdesk-server:<커밋12자리>
```

## 배포 후 확인

- `hbbs` 로그의 `Key:`가 기존 키(`18FnRRLE…`)와 같은지 확인한다. 같아야 클라이언트 설정을 바꾸지 않아도 된다.
- 시작할 때 `Failed to get machine uid` 오류가 한 줄 뜨는 것은 알려진 것이고 무시해도 된다.
- Change ID는 테스트 PC로 먼저 해 보고, 그다음 LTE 차량에서 해 본다. 차량은 IP가 바뀌어 거부될 수 있다.
