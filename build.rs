fn main() {
    // gen_version() 은 Cargo.toml 만 감시하므로, src/version.rs 가 없으면
    // (새 체크아웃 + target 캐시) build.rs 가 다시 돌지 않는다. 직접 감시를 추가한다.
    println!("cargo:rerun-if-changed=src/version.rs");
    hbb_common::gen_version();
}
