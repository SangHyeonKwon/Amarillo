# M001 — Failure Intelligence Core API · ROADMAP

GSD-2 계층: Milestone → Slice(데모 가능한 수직 기능) → Task(컨텍스트 1개 크기).
배지: `[edge: untapped]` Dune이 전혀 안 함(최우선) · `[edge: weak-spot]` Dune이 약함 ·
`[sketch]` 미정제(다음 Reassess에서 확장).

순서 원칙: **엣지 우선순위 = untapped → weak-spot**, 단 하드 의존성은 존중.

---

## M001 — Failure Intelligence Core API  ✅ SHIPPED → M001-SUMMARY.md
출하 정의: REQUIREMENTS.md#M001. "임의 실패 tx를 조회·진단, 외부 임베드 가능."
수용 기준 전 항목 ✅, 최종 게이트 green (clippy/fmt/`cargo test -p db --ignored` 8/8/verify ALL PASS).

- [x] **S01 — 단건 실패 진단 엔드포인트** `[edge: untapped]` · risk: low · **DONE** → S01-SUMMARY.md
  - `GET /v1/failed-tx/{tx_hash}` 출하. db 쿼리 2개 + api 라우트 + 검증 스크립트/문서
  - 게이트 통과: clippy 0 · fmt OK · `verify-failed-tx.sh` green · prod unwrap 0
  - 산출: `crates/db`(get_failed_transaction, list_trace_logs_by_tx, FailedTxDetail),
    `crates/api/src/routes/failed_tx.rs`, `scripts/verify-failed-tx.sh`, `docs/api-failed-tx.md`

- [x] **S02 — 실패 목록/검색 API** `[edge: weak-spot]` · risk: low · **DONE** → S02-SUMMARY.md
  - `GET /v1/failed-tx?category=&from=&to=&limit=&offset=` + 정확한 `total` 출하
  - 산출: `TotalPaginatedResponse`/`PaginationMeta`(response.rs), `ErrorCategory: FromStr`,
    `list_failed_tx` 핸들러, 통합테스트 2건, 스크립트/문서 확장
  - 게이트: clippy 0 · fmt OK · verify(단건+목록+400) green · `cargo test -p db --ignored` 6/6
  - `contract` 필터 미구현(`[sketch]`, transaction.to_addr 조인 — S04/별도)

- [x] **S03 — 실패 추이 시계열** `[edge: weak-spot]` · risk: med · **DONE** → S03-SUMMARY.md
  - `GET /v1/analytics/failed-tx/timeseries?interval=&from=&to=` 출하 (카테고리×버킷)
  - 산출: `TimeBucket`(+FromStr,+as_pg), `FailedTxTrendPoint`, `failed_tx_timeseries`,
    핸들러, 통합테스트(재조정/단조), 스크립트/문서
  - 인젝션 안전: enum 화이트리스트 + `date_trunc($1,..)` 바인딩 · 게이트 green

- [x] **S04 — 임베드 가능화 + 하드닝** `[edge: untapped]` · risk: low · **DONE** → S04-SUMMARY.md
  - L1 명시컬럼 / L2 tx_hash 형식 400 / L3 call_tree 상한(N+1 잘림감지·`call_tree_truncated`)
  - 통합 API 레퍼런스(`docs/api-failed-tx.md`)+README 포인터, M001 수용 전체 검증 통과

---

## M002 — Real-time Failure Pipeline  ✅ SHIPPED → M002-SUMMARY.md
출하 정의: REQUIREMENTS.md#M002. "새 블록 실패가 수 초 내 조회 가능, reorg에도 정합."
수용 기준 전 항목 ✅, 최종 게이트 green (fmt clean · clippy --workspace 0 ·
`cargo test -p indexer` 18/18 · `-p db --ignored` 9/9).

- [x] **S05 — 체인 헤드 팔로워** `[edge: untapped]` · risk: high · **DONE** → S05-SUMMARY.md
  - `--follow`/`--poll-interval-secs`/`--confirmations`, `WorkerPool::follow`,
    순수 `next_target`(단위테스트 5), graceful ctrl_c. 스코프 D009.
  - 게이트: `cargo test -p indexer` 5/5 · clippy 0 · fmt clean · 비-follow 무회귀
- [x] **S06 — Reorg 감지·정정** `[edge: untapped]` · risk: high · **DONE** → S06-SUMMARY.md
  - 마이그레이션(block hash) + 순수 `find_fork_point`(단위6) + 멱등 `rollback_from_block`
    (통합1) + follow 결선 `detect_fork`. 안전규칙: 불확실 RPC→무롤백. D010.
  - 게이트: `cargo test -p indexer` 11/11 · `-p db --ignored` 9/9 · clippy 0 · fmt clean
- [x] **S07 — 실시간 하드닝/관측성** `[edge: weak-spot]` · risk: med · **DONE** → S07-SUMMARY.md
  - T01 관측성(사이클당 구조화 1줄) + T02 `eth_subscribe`(D011, 폴백 무회귀) +
    T03 lazy+동적확대(`classify_fork`/`next_scan_depth`/cap 4096 — 리뷰 R1 under-delete
    갭 제거·R2 prefetch 해소). 신규 의존성 0. R3/R4 하드닝 백로그.
  - 게이트: `cargo test -p indexer` 18/18 · `-p db --ignored` 9/9 · clippy 0 · fmt clean

## M003 — Actionable Alerts  ← M002 출하로 분해 시작 (S08 분해 완료)
출하 정의: REQUIREMENTS.md#M003. M003 = S08 ∧ S09.

- [ ] **S08 — 실패 패턴 구독 + 웹훅 전송** `[edge: untapped]` · risk: med · deps: M002 · **다음 → S08-PLAN.md (분해 완료)**
  - T01 alert 스키마/모델/매칭 쿼리(멱등 마이그레이션 + 통합테스트) → T02 디스패처
    (순수 SSRF 가드·HMAC 서명·매칭 + 얇은 전송 드라이버, `indexer --dispatch-alerts`,
    멱등 `alert_delivery`) → T03 구독 관리 API + 검증/문서 + S09 reassess. D012.
- [ ] **S09 — 온체인 × 비공개 데이터 조인 예시** `[edge: untapped]` `[sketch]` · deps: M001, S08
  - 가장 방어 가능한 해자. 소비 유스케이스 확정 후 정제(S08 출하 시점). M003 출하 = S08 ∧ S09.

---

## 백로그 (이월 · 우선순위 미정)

- [x] **TEST-HARNESS** — `crates/db` cargo 통합테스트 하니스. **DONE** → STH-SUMMARY.md
      (`crates/db/tests/failed_tx.rs` 4건, `cargo test -p db -- --ignored`, D007 RESOLVED)
- [x] **S04 하드닝 항목 (리뷰 L1–L3)**: S04에서 해소(명시 컬럼·tx_hash 400·call_tree 상한).
- [ ] **FE-WIRE (R2)**: 대시보드(`web/`)가 실패-인텔리전스 API(S01–S03)를 아직 소비 안 함
      (contract.ts가 S01 이전 작성). M001은 API-first라 결함 아님. 향후 프론트 슬라이스로
      `/v1/failed-tx*` 연동(드릴다운 UX). M002 이후 우선순위 재평가.

## Reassess 규칙 (GSD-2)
각 슬라이스 Complete 후 이 ROADMAP 갱신: 다음 슬라이스 `[sketch]` 해제·태스크 분해,
새 Lesson은 KNOWLEDGE.md, 방향 변경은 DECISIONS.md. M003은 M002 출하 전 분해 금지
(M001·M002 출하 완료 → 이제 M003 분해 가능).
