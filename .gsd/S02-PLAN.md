# S02 — 실패 목록/검색 API · PLAN

Slice 목표: `GET /v1/failed-tx?category=&from=&to=&limit=&offset=` — 실패 트랜잭션을
필터·페이지네이션으로 조회하고 **응답에 정확한 `total`**(D005)을 포함. 임베드형 제품의
기본기(서버사이드 필터 + 정확한 페이지 메타).

엣지: `[edge: weak-spot]` — Dune에서도 가능하나 임베드용 API 계약(정확 total, 저지연 필터)은
약점. S01의 단건 진단과 함께 "목록→단건 드릴다운" UX의 절반.

전제: S01 완료(`get_failed_transaction` 등 존재). 마이그레이션 불필요(기존 테이블만).
범위 제외: `contract` 필터(=transaction.to_addr 조인) → 별도 `[sketch]`.

태스크 순서: T01 → T02 → T03.

---

## T01 — DB: 필터형 목록 + 카운트 쿼리

**Must-haves**
- *Truths*
  - `list_failed_transactions(pool, category: Option<ErrorCategory>, from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>, limit, offset)` → 조건에 맞는 `FailedTransaction` 목록,
    `timestamp DESC` 정렬
  - `count_failed_transactions(pool, category, from, to)` → 동일 필터의 전체 건수(i64)
  - 필터 미지정 시 전체 대상; 빈 결과는 빈 `Vec`/`0`(에러 아님)
- *Artifacts*
  - `crates/db/src/queries.rs`: 위 2 `pub async fn` (`///`doc, `($1::... IS NULL OR col=$1)`
    옵션 필터 패턴 = 기존 `list_swap_events` 모방, `?` 전파, no unwrap)
  - `ErrorCategory`는 기존 `error_category_to_sql` 헬퍼로 바인딩
- *Key Links*
  - 기존 `FailedTransaction`(FromRow), `DbError`, `list_swap_events`의 옵션필터/페이지 패턴 재사용
- *검증*: `cargo check -p db` + 통합 호출(시드 카테고리=UNKNOWN, 기간 필터) 행수 일치

## T02 — API: total 포함 페이지 응답 + `GET /v1/failed-tx`

**Must-haves**
- *Truths*
  - `curl '/v1/failed-tx?category=UNKNOWN&limit=2'` → `200`, `data[]`(≤2) +
    `pagination.total`(필터 적용 전체 건수, 현재 페이지 길이 아님)
  - `from`/`to`(ISO8601) 필터 동작; 잘못된 enum/날짜 → `400 {error}`
  - 기존 `/v1/failed-tx/{tx_hash}`(S01)와 라우트 충돌 없음(서로 다른 경로)
- *Artifacts*
  - `crates/api/src/response.rs`: `TotalPaginatedResponse<T> { data, pagination: { limit,
    offset, total } }` (신규 — 기존 `PaginatedResponse`는 계약호환 위해 불변, D005)
  - `crates/api/src/routes/failed_tx.rs`: `list_failed_tx` 핸들러(Query 파라미터 구조체)
  - `crates/api/src/routes/mod.rs`: `.route("/failed-tx", get(failed_tx::list_failed_tx))`
- *Key Links*
  - 기존 `PaginationParams`(limit clamp), `ApiError::BadRequest`(400) 재사용
  - T01의 list/count를 한 핸들러에서 조합해 total 산출
- *검증*: 라이브(:3001) 필터·페이지·total 단언

## T03 — 검증 자산 갱신

**Must-haves**
- *Truths*: `scripts/verify-failed-tx.sh`에 목록 케이스 추가(필터+total 단언), green 유지
- *Artifacts*: 스크립트 확장, `docs/api-failed-tx.md`에 `GET /v1/failed-tx` 섹션 추가
- *Key Links*: S01 스크립트/문서 구조 그대로 확장

---

## Slice 수용 (Complete 게이트)
- [ ] T01·T02·T03 must-haves 충족
- [ ] `cargo clippy -p db -p api -- -D warnings` / `cargo fmt --check` 통과
- [ ] `verify-failed-tx.sh` green (단건+목록)
- [ ] Complete 후 Reassess: ROADMAP S02 체크, S03 `[sketch]` 해제·분해, Lesson→KNOWLEDGE
