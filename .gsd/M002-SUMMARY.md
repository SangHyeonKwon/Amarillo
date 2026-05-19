---
milestone: M002
title: Real-time Failure Pipeline
status: SHIPPED
date: 2026-05-20
slices: [S05, S06, S07]
gate: pass   # fmt --check clean · clippy --workspace -D warnings 0 · cargo test -p indexer 18/18 · -p db --ignored 9/9
decisions: [D009, D010, D011]
---

# M002 — Real-time Failure Pipeline · SHIPPED

출하 정의(REQUIREMENTS#M002): **"새 블록의 실패가 수 초 내 조회 가능하고, reorg에도
데이터가 정합적이다."**

## 수용 기준 — 항목별 ✅ (증빙)

- **인덱서가 체인 헤드를 따라가며 신규 블록 실패를 자동 분류·저장 (연속 루프)** ✅
  - S05: `--follow`/`--poll-interval-secs`/`--confirmations`, `WorkerPool::follow`
    연속 루프, 순수 `next_target`, graceful ctrl_c. 실패 분류·저장은 기존
    `process_block`→`decoder::classifier` 경로 재사용(M001 자산).
  - S07-T01: 사이클당 구조화 관측(lag/처리량/reorg깊이) — 운영 가능성 확보.
  - 증빙: `next_target` 단위 5, follow 결선 컴파일/clippy, 라이브 수동 스모크(docs).
- **reorg 발생 시 영향 블록 데이터 정정, 멱등성 유지, 중복/유령 행 없음** ✅
  - S06: `block_hash`/`parent_hash` 멱등 마이그레이션, 순수 fork 탐지, 단일
    트랜잭션 멱등 `rollback_from_block`(FK 역순, checkpoint 되감기).
  - S07-T03: lazy + 동적 확대(`classify_fork`/`next_scan_depth`/cap 4096)로
    **진짜 최소 공통조상**까지 정정 → 과거 64-윈도우의 under-delete(유령행)
    갭 제거. 안전규칙: 체인 해시 불확실 시 무롤백(불변).
  - 증빙: `crates/db/tests/rollback.rs` 멱등·스코프 통합테스트 green(9/9 중 1),
    순수 판정 단위 7(경계: NeedDeeper→floor, 종료성).
- **수 초 내 조회 가능** ✅(정직 단서): S07-T02 `--subscribe`(eth_subscribe
  newHeads, D011)로 폴 간격 지연 제거(헤드 푸시 즉시 트리거). 폴링 기본 시엔
  `--poll-interval-secs`+`--confirmations` 합산 지연(D009 트레이드오프, 문서화).
- **공통 비기능** ✅: prod `unwrap()` 0(clippy `-D warnings` green), 파라미터화
  SQL(sqlx bind), 신규 마이그레이션 멱등(S06), 신규 public `///` 문서, 통합
  검증(`-p db --ignored` 9/9). 시크릿 안전: WS_URL env 전용·로그 미출력.

## 최종 게이트 (2026-05-20, 새로 재실행 — KNOWLEDGE S04 규칙)

- `cargo fmt --check` (workspace) — clean
- `cargo clippy --workspace -- -D warnings` — 0 (db/api/indexer)
- `cargo test -p indexer` — **18/18** (next_target 5 + classify_fork/next_scan_depth 7 + resolve_trigger_mode 6)
- `cargo test -p db -- --ignored` — **9/9** (failed_tx 8 + rollback 멱등·스코프 1)

## 슬라이스

- **S05** 체인 헤드 팔로워 `[untapped]` — follow 루프 + 순수 next_target. (D009)
- **S06** Reorg 감지·정정 `[untapped]` — hash 마이그레이션 + 멱등 rollback. (D010)
- **S07** 실시간 하드닝/관측성 `[weak-spot]` — 관측성 + eth_subscribe(D011) +
  동적 확대(R1/R2 해소). → S07-SUMMARY.md

## 정직한 한계 / 잔여

- 라이브 follow/subscribe/reorg는 `RPC_URL`/`WS_URL` 필요 — 이 환경 미보장.
  순수 결정 로직(18 단위) + rollback 통합(9/9) + 컴파일/clippy가 1차 증빙,
  라이브 경로는 `docs/realtime-follow.md` 수동 스모크 절차로 위임(D009/D010/D011
  검증 제약 동일).
- 정확성 경계: **4096블록 초과** reorg만 best-effort floor 롤백 — PoS finality
  ≈64의 64배 마진, 사실상 발생 불가. 과거 "보수적=안전"이라는 *미명시 가정*을
  S07-T03가 **명시된 경계**로 대체(리뷰 R1 자가발견·정정·실현).
- 하드닝 백로그(정확성 결함 아님): R3 per-iteration 범위 cap, R4 `index_range`
  진행 중 ctrl_c 응답성 — S07-PLAN 백로그.

## Reassess

ROADMAP M002 `[x] SHIPPED`. M003(Actionable Alerts, `[sketch]`)은 GSD-2 규칙대로
**다음 지시 시** S08/S09 분해(출하 전 분해 금지 → 이제 분해 가능). 차별화 해자
순위: S09(온체인×비공개 조인 예시)·S08(구독/웹훅). FE-WIRE 백로그 우선순위도
M002 출하로 재평가 시점.
