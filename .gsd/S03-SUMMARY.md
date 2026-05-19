---
slice: S03
title: 실패 추이 시계열
status: done
edge: weak-spot
tasks: [T01, T02, T03]
gate: pass            # clippy 0, fmt ok, verify(단건+목록+추이+400) green, cargo test -p db --ignored 7/7
migrations: none
artifacts:
  - crates/db/src/models.rs            # + TimeBucket(+FromStr,+as_pg), FailedTxTrendPoint
  - crates/db/src/queries.rs           # + failed_tx_timeseries
  - crates/db/tests/failed_tx.rs       # + reconcile/ordered 통합테스트
  - crates/api/src/routes/failed_tx.rs # + failed_tx_timeseries 핸들러, FailedTxTrendQuery
  - crates/api/src/routes/mod.rs       # + /analytics/failed-tx/timeseries
  - scripts/verify-failed-tx.sh        # + timeseries(200/정렬/400)
  - docs/api-failed-tx.md              # + timeseries 섹션
endpoint: "GET /v1/analytics/failed-tx/timeseries?interval=&from=&to="
---

# S03 — 무엇이 실제로 일어났나 (계획 대비)

S03-PLAN T01→T02→T03 그대로. 마이그레이션 없음.

**계획대로 된 것**
- DB: `failed_tx_timeseries` — `date_trunc($1, timestamp)`에 `TimeBucket` enum→고정
  리터럴을 **바인딩**(문자열 보간 0), `GROUP BY 1,2 / ORDER BY 1,2`. 통합테스트로
  재조정 불변식(버킷 합 == `count_failed_transactions` 전체) + 버킷 단조 검증 →
  `cargo test -p db --ignored` 7/7.
- API: S02의 `parse_ts` + `FromStr→400` 패턴 재사용 → 핸들러가 얇음.
  `?interval=bogus` → 400, `?interval=day` → 200(points=1).
- 검증/문서 확장, S01·S02 무회귀.

**배운 것 / 패턴화 (→ KNOWLEDGE, DECISIONS)**
- 인젝션 안전 동적 집계를 **Rule**로 승격(enum 화이트리스트 + 바인딩 + ordinal GROUP BY).
- S04 OpenAPI는 프레임워크 대신 손작성(**D008**) — 의존성 침투 회피, ROI 우위.
- 검증 경계/얇은 핸들러 패턴이 S02→S03에서 재사용되며 신규 슬라이스 비용이 체감 감소.

**Reassess**: ROADMAP S03 `[x]`, S04 `[sketch]` 해제·S04-PLAN.md 분해 완료.
S04는 M001 마무리 슬라이스(하드닝 L1–L3 + 임베드 + Milestone Validate).
M002/M003은 M001 출하 전 분해 금지 유지.
