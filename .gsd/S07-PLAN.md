# S07 — 실시간 하드닝/관측성 (M002 마무리) · PLAN

Slice 목표: follow 파이프라인을 운영 가능한 수준으로 — 관측성, `eth_subscribe`
옵션(D009 이월), reorg 윈도우 동적 확대(D010 이월) — 그리고 M002 출하 검증.

엣지: `[edge: weak-spot]`. risk: med. deps: S06.
검증 제약(D009/D010 동일): WS/RPC 필요분은 순수 로직 단위테스트 + 수동 스모크.
태스크: T01 → T02 → T03.

---

## T01 — 관측성 (메트릭/구조화 로그)

**Must-haves**
- *Truths*
  - follow 매 사이클: 인덱싱 lag(`head - checkpoint`), 처리 블록 수, reorg 발생/깊이,
    마지막 폴 시각을 구조화 `tracing` 필드로 방출(주기적 요약 로그 1줄 포함)
  - 기존 동작/성능 회귀 없음, 신규 의존성 없음(`tracing` 재사용)
- *Artifacts*: `crates/indexer/src/worker.rs`(follow에 경량 카운터/요약 로그),
  `docs/realtime-follow.md` 관측성 절
- *Key Links*: 기존 `tracing::{info,warn,debug}`, follow 루프

## T02 — `eth_subscribe` 옵션 (폴링 대안)

**Must-haves**
- *Truths*
  - `--subscribe`(+ `WS_URL`) 시 newHeads 구독으로 사이클 트리거(폴링 sleep 대체).
    `WS_URL` 없거나 미지정 → 기존 폴링으로 폴백(회귀 없음)
  - reorg 체크/`next_target`/index 로직은 그대로 재사용(트리거만 교체)
  - 구독 드라이버의 순수 부분(트리거→액션 결정)은 RPC 없이 테스트, 라이브는 수동
- *Artifacts*: `config.rs`/`main.rs`(`--subscribe`), `worker.rs`(ws 트리거 분기,
  `ws_url` 활용 — 기존 `#[allow(dead_code)] ws_url` 해소), docs
- *Decision*: D011 — 폴링 기본, 구독 옵트인(WS 가용성 편차). (착수 시 DECISIONS 기록)

## T03 — reorg 윈도우 동적 확대 + M002 Milestone Validate

**Must-haves**
- *Truths*
  - `find_fork_point`가 윈도우 floor(전부 불일치)를 반환하면 스캔 폭을 cap까지
    배수 확대해 **최소 공통조상**을 찾는다(과삭제 축소). 순수 로직 → 단위테스트
    (경계: cap 도달, 점진 확대). 안전규칙(불확실→무롤백) 유지
  - REQUIREMENTS#M002 수용 기준 항목별 ✅ + 전체 게이트 green
- *Artifacts*: `worker.rs`(확대 로직 + 단위테스트), `.gsd/S07-SUMMARY.md`,
  `.gsd/M002-SUMMARY.md`, ROADMAP M002 `[x]`
- *Reassess*: M002 출하 후에만 M003(`[sketch]`) 분해 착수(GSD-2)

---

## Slice 수용 (Complete = M002 출하)
- [ ] T01–T03 must-haves, 폴링 회귀 없음
- [ ] `cargo test -p indexer`(순수 확대 로직 포함) green · `-p db --ignored` 9/9 · clippy/fmt
- [ ] REQUIREMENTS#M002 수용 기준 전체 ✅, M002-SUMMARY, ROADMAP M002 마감
- [ ] 라이브(follow/subscribe/reorg) 수동 스모크 절차 문서화(RPC/WS 부재 시 단위테스트 1차 증빙)
