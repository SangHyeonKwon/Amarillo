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

## M002 — Real-time Failure Pipeline `[sketch]`  ← M001 출하로 분해 가능 (다음 지시 시 S05-PLAN)
출하 정의: REQUIREMENTS.md#M002.

- [ ] **S05 — 체인 헤드 팔로워** `[edge: untapped]` `[sketch]` · risk: high · deps: M001
  - 학습: head vs finalized, polling vs `eth_subscribe`, 연속 인덱싱 루프, 백프레셔
- [ ] **S06 — Reorg 감지·정정** `[edge: untapped]` `[sketch]` · risk: high · deps: S05
  - 학습: finality, 멱등 롤백 — 실무 인덱서 최난제
- [ ] **S07 — 실시간 하드닝/관측성** `[sketch]` · deps: S06

## M003 — Actionable Alerts `[sketch]`
출하 정의: REQUIREMENTS.md#M003.

- [ ] **S08 — 구독 모델 + 웹훅 전송** `[edge: untapped]` `[sketch]` · deps: M002
- [ ] **S09 — 온체인 × 비공개 데이터 조인 예시** `[edge: untapped]` `[sketch]` · deps: M001
  - 가장 방어 가능한 해자. 소비 유스케이스 확정 후 정제.

---

## 백로그 (이월 · 우선순위 미정)

- [x] **TEST-HARNESS** — `crates/db` cargo 통합테스트 하니스. **DONE** → STH-SUMMARY.md
      (`crates/db/tests/failed_tx.rs` 4건, `cargo test -p db -- --ignored`, D007 RESOLVED)
- [ ] **S04 하드닝 항목 (리뷰 L1–L3)**: `SELECT *` → 명시 컬럼, 잘못된 tx_hash 형식 400,
      `call_tree` 크기 상한/페이지네이션. S04에서 일괄 처리.

## Reassess 규칙 (GSD-2)
각 슬라이스 Complete 후 이 ROADMAP 갱신: 다음 슬라이스 `[sketch]` 해제·태스크 분해,
새 Lesson은 KNOWLEDGE.md, 방향 변경은 DECISIONS.md. M002/M003은 M001 출하 전 분해 금지.
