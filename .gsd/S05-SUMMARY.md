---
slice: S05
title: 체인 헤드 팔로워
status: done
edge: untapped
milestone: M002
tasks: [T01, T02, T03]
gate: pass            # cargo test -p indexer 5/5 (next_target), clippy 0, fmt clean, no prod unwrap
migrations: none
artifacts:
  - crates/indexer/src/config.rs   # follow/poll_interval_secs/confirmations + with_follow_opts
  - crates/indexer/src/main.rs     # --follow/--poll-interval-secs/--confirmations, follow 분기
  - crates/indexer/src/worker.rs   # WorkerPool::follow + pure next_target + unit tests
  - docs/realtime-follow.md        # 사용법·한계(D009)·수동 스모크
scope: "poll + (head - confirmations); eth_subscribe=S07, deep reorg=S06 (D009)"
---

# S05 — 무엇이 실제로 일어났나 (계획 대비)

S05-PLAN T01(CLI/Config)→T02(루프+next_target)→T03(검증/문서) 그대로.

**계획대로 된 것**
- T01: `--follow`/`--poll-interval-secs`/`--confirmations`, `--from-block` Optional
  (follow는 checkpoint가 재개 진실). 비-follow 경로 **불변**(무인자 → 명확한 bail).
- T02: 루프의 결정 로직을 순수 함수 `next_target(head,conf,checkpoint)`로 분리 →
  **RPC 없이 단위테스트 5건**(경계 포함) green. 드라이버는 `tokio::select!`로
  `ctrl_c` graceful 종료. 기존 `index_range`/`rpc_with_retry`/checkpoint 재사용.
- T03: clippy 0·fmt clean, `docs/realtime-follow.md`(D009 한계·수동 스모크).

**설계 판단 (KNOWLEDGE 반영)**
- IO 루프에서 **순수 결정 로직 분리** → 환경 의존(RPC) 없이 회귀 테스트 가능.
  D009의 "검증 제약"을 이 분리로 충족.
- follow는 **백필 안 함**: 체크포인트 없으면 safe tip부터(전체 체인 안 긁음).
- 약간의 의도적 PLAN 이탈: Config는 `poll_interval_secs: u64`로 보관(기존 Config가
  전부 primitive), `Duration`은 호출부에서 생성 — 일관성/단순성.

**검증 한계 (정직)**: 라이브 follow는 `RPC_URL` 필요. 이 환경엔 RPC 미보장 →
1차 증빙은 `next_target` 단위테스트, 라이브는 문서화된 수동 스모크.

**Reassess**: ROADMAP S05 `[x]`, S06 `[sketch]` 해제·S06-PLAN.md 분해. S07 `[sketch]` 유지.
