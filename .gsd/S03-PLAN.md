# S03 — 실패 추이 시계열 · PLAN

Slice 목표: `GET /v1/analytics/failed-tx/timeseries?interval=&from=&to=` —
실패 건수를 **시간 버킷 × 에러 카테고리**로 집계. 임베드형 차트의 데이터 소스.

엣지: `[edge: weak-spot]` — Dune SQL로도 가능하나, 저지연 API 계약으로 임베드되는 형태는
약점. S01 단건 + S02 목록에 이은 "추세" 축.

전제: S02 완료(`ErrorCategory: FromStr`, `parse_ts`, 검증 패턴 재사용). 마이그레이션 불필요.
태스크: T01 → T02 → T03.

---

## T01 — DB: 버킷 집계 쿼리 + 통합테스트

**Must-haves**
- *Truths*
  - `failed_tx_timeseries(pool, bucket: &TimeBucket, from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>)` → `Vec<FailedTxTrendPoint{ bucket: DateTime<Utc>,
    error_category: ErrorCategory, failure_count: i64 }>`, `bucket ASC, error_category` 정렬
  - 카테고리별 합 = `count_failed_transactions(category, from, to)` (재조정 불변식)
  - 빈 구간 → 빈 `Vec`(에러 아님)
- *Artifacts*
  - `crates/db/src/models.rs`: `enum TimeBucket { Hour, Day, Week }` (+ `FromStr`, `as_pg()` →
    `'hour'|'day'|'week'` 고정 리터럴) + `FailedTxTrendPoint`(FromRow, Serialize)
  - `crates/db/src/queries.rs`: `failed_tx_timeseries` — `date_trunc($1, timestamp)` 에
    **bucket을 바인딩 파라미터로** 전달(절대 문자열 보간 금지; 화이트리스트 enum→고정 텍스트)
  - `crates/db/tests/failed_tx.rs`: 재조정 불변식 + 단조 bucket 통합테스트(`#[ignore]`)
- *Key Links*: 기존 `error_category_to_sql`, `FailedTransaction.timestamp`, S02 옵션필터 관용구

## T02 — API: timeseries 라우트

**Must-haves**
- *Truths*
  - `?interval=day`(기본) 200, 데이터 정렬됨; `?interval=bogus` → **400**;
    `?from=not-a-date` → 400 (S02 `parse_ts` 재사용)
  - 응답: `ApiResponse<Vec<FailedTxTrendPoint>>` (목록형, 페이지네이션 불요 — 버킷 수 유한)
- *Artifacts*
  - `crates/api/src/routes/failed_tx.rs`: `failed_tx_timeseries` 핸들러 + Query 구조체
    (`interval`/`from`/`to`), `TimeBucket: FromStr`로 400 매핑
  - `crates/api/src/routes/mod.rs`: `.route("/analytics/failed-tx/timeseries", get(...))`
- *Key Links*: `ApiError::BadRequest`, S02 `parse_ts`, `ApiResponse`

## T03 — 검증 + 문서 + Reassess

**Must-haves**
- *Truths*: `verify-failed-tx.sh`에 timeseries 케이스(200 + 정렬 + 400) 추가, green 유지
- *Artifacts*: 스크립트 확장, `docs/api-failed-tx.md` timeseries 섹션, S03-SUMMARY.md
- *Reassess*: ROADMAP S03 `[x]`, S04 `[sketch]` 해제·분해, Lesson→KNOWLEDGE

---

## Slice 수용 (Complete 게이트)
- [ ] T01·T02·T03 must-haves 충족
- [ ] `cargo clippy -p db -p api --tests -- -D warnings` / `cargo fmt --check`
- [ ] `verify-failed-tx.sh`(단건+목록+추이+400) green · `cargo test -p db -- --ignored` green
- [ ] 인젝션 안전: interval은 enum 화이트리스트만 (절대 문자열 보간 금지)
