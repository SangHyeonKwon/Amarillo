# S05 — 체인 헤드 팔로워 · PLAN

Slice 목표: 인덱서가 고정 범위가 아니라 **체인 헤드를 따라가며** 신규 블록을 연속
인덱싱(기존 실패-tx 파이프라인 그대로 통과). 기존 `index_range` + checkpoint 재사용.

엣지: `[edge: untapped]` — Dune은 구조상 실시간 불가. M002의 첫 슬라이스.
스코프(D009): **poll + (head − confirmations)까지만**. `eth_subscribe`=S07, 깊은 reorg=S06.
risk: high (RPC 의존, 장기 실행 루프, 얕은 reorg 노출은 confirmations로 완화).

전제: M001 출하. 마이그레이션 없음(checkpoint 테이블 기존). 태스크: T01 → T02 → T03.

---

## T01 — CLI/Config: follow 모드

**Must-haves**
- *Truths*
  - `Cli`에 `--follow`(flag), `--poll-interval-secs`(기본 12), `--confirmations`(기본 12)
  - `--follow`이면 `--to-block` 무시; `--from-block` 없이도 동작(checkpoint/기본에서 시작)
  - `--follow` 없으면 기존 범위 인덱싱 동작 **불변**(회귀 없음)
- *Artifacts*
  - `crates/indexer/src/config.rs`: `Config`에 `follow: bool, poll_interval: Duration,
    confirmations: u64` (+ `///`), `from_env`/오버라이드 반영
  - `crates/indexer/src/main.rs`: `clap` `Cli`에 필드 추가, 분기 배선
- *Key Links*: 기존 `Config::from_env`, `with_block_range`, `get_last_checkpoint`
- *검증*: `cargo run -p indexer -- --help` 출력 확인 + `cargo build -p indexer`

## T02 — follow 루프 + 순수 타깃 로직

**Must-haves**
- *Truths*
  - 순수 함수 `next_target(head, confirmations, last_checkpoint) -> Option<(u64,u64)>`:
    안전 헤드 `safe = head.saturating_sub(confirmations)`; `checkpoint+1 > safe` → `None`
    (인덱싱할 새 블록 없음); 아니면 `Some((checkpoint+1, safe))`
  - 드라이버 루프: head 조회(`rpc_with_retry`) → `next_target` → `Some`면
    `index_range`(기존, 청크별 checkpoint 갱신) → `poll_interval` sleep → 반복
  - `ctrl_c`에 **graceful 종료**(진행 중 청크 완료 후 정지, 패닉/유실 없음)
- *Artifacts*
  - `crates/indexer/src/worker.rs`: `WorkerPool::follow(...)` + `next_target`(pub, 테스트용)
    + `#[cfg(test)] mod tests`(순수 로직 단위테스트, RPC 불요)
  - `crates/indexer/src/main.rs`: `config.follow`면 `follow()` 호출
- *Key Links*: `index_range`, `rpc_with_retry`, `get_last_checkpoint`,
  `tokio::signal::ctrl_c`, `tokio::select!`
- *검증*: `cargo test -p indexer`(next_target 단위테스트 green, RPC 불요)

## T03 — 검증/문서 + Reassess

**Must-haves**
- *Truths*
  - `cargo test -p indexer` green, `cargo clippy -p indexer -- -D warnings`, `fmt --check`
  - 라이브 follow는 `RPC_URL` 필요 → 수동 스모크 절차 문서화(가정·한계 명시)
- *Artifacts*: README/CLAUDE 또는 docs에 follow 사용법 + D009 한계,
  `.gsd/S05-SUMMARY.md`, ROADMAP S05 `[x]`, S06 `[sketch]` 해제·분해
- *Key Links*: D009(검증 제약), KNOWLEDGE Lesson(실시간 루프/graceful shutdown)

---

## Slice 수용 (Complete 게이트)
- [ ] T01·T02·T03 must-haves 충족, 기존 범위 인덱싱 회귀 없음
- [ ] `cargo test -p indexer`(순수 로직) green · clippy/fmt 통과 · prod unwrap 0
- [ ] 라이브 follow 수동 스모크 절차 문서화(RPC 부재 시 단위테스트가 1차 증빙)
- [ ] Reassess: S06(reorg) `[sketch]` 해제·분해
