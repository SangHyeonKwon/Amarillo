# S01 — 단건 실패 진단 엔드포인트 · PLAN

Slice 목표: `GET /v1/failed-tx/{tx_hash}` → 디코딩된 revert 사유 + `error_category` +
평탄화 콜트리(D004)를 반환. 데이터는 이미 `failed_transaction` / `trace_log`에 존재 →
**순수 추가, 마이그레이션 불필요, 프론트 제외(후순위)**.

엣지: `[edge: untapped]` — Dune은 "이 해시 왜 실패?"를 분류된 형태로 out-of-box 제공 안 함.

태스크는 컨텍스트 1개 크기. 순서: T01 → T02 → T03.

---

## T01 — DB: 단건 실패 + 콜트리 읽기 쿼리

`crates/db`에 진단 단건 조회를 추가. 기존 `queries.rs` 패턴 그대로.

**Must-haves**
- *Truths*
  - `get_failed_transaction(pool, tx_hash)` → 시드된 실패 해시로 호출 시 `FailedTransaction` 반환;
    미존재 해시 → `DbError::NotFound`
  - `list_trace_logs_by_tx(pool, tx_hash)` → 해당 tx의 `TraceLog`들을 `call_depth, trace_id` 오름차순 반환;
    트레이스 없으면 빈 `Vec`(에러 아님)
- *Artifacts*
  - `crates/db/src/queries.rs`: 위 두 `pub async fn` (각 `///` doc, `sqlx::query_as`,
    파라미터 바인딩, `?` 에러 전파 — `unwrap()` 금지)
  - `crates/db/src/models.rs`: 응답 조립용 `FailedTxDetail { failed: FailedTransaction,
    call_tree: Vec<TraceLog> }` (`serde::Serialize`)
- *Key Links*
  - 기존 `FailedTransaction` / `TraceLog` (`FromRow`) 재사용, `DbError::NotFound` 재사용
  - `get_pool_by_address`의 `fetch_optional().ok_or_else(NotFound)` 패턴 모방

**검증**: `cargo test -p db -- --ignored` 통합 테스트 1개 (시드 해시 → Ok, 랜덤 해시 → NotFound)

---

## T02 — API: `GET /v1/failed-tx/{tx_hash}` 라우트

`crates/api`에 엔드포인트 노출. 기존 라우트/봉투/에러 규약 준수.

**Must-haves**
- *Truths*
  - 시드된 실패 해시로 `curl :3000/v1/failed-tx/{hash}` → `200`,
    `{ "data": { "failed": {...,"error_category":...,"revert_reason":...}, "call_tree": [...] } }`
  - 미존재/형식불량 해시 → `404` 바디 `{ "error": ... }`
  - 응답이 기존 `ApiResponse<T>` 봉투 형태와 동일
- *Artifacts*
  - `crates/api/src/routes/failed_tx.rs`: 핸들러 (`Path(tx_hash)`, `State<PgPool>`,
    `Result<Json<ApiResponse<FailedTxDetail>>, ApiError>`, `///` doc)
  - `crates/api/src/routes/mod.rs`: `v1_router()`에 `.route("/failed-tx/{tx_hash}", get(...))`
    + `pub mod failed_tx;`
- *Key Links*
  - `error.rs`의 `From<DbError> for ApiError` (NotFound→404) 자동 사용
  - T01의 `db::queries::get_failed_transaction` + `list_trace_logs_by_tx` 조립
  - `response::ApiResponse` 재사용

**검증**: 스택 기동 상태에서 시드 해시 curl 성공 + 랜덤 해시 404 (스크립트는 T03)

---

## T03 — 검증 자산: 재현 스크립트 + 엔드포인트 문서

임베드 가능 증빙 (REQUIREMENTS.md#M001 수용 기준).

**Must-haves**
- *Truths*
  - `scripts/verify-failed-tx.sh` 실행 시 시드 해시 200·랜덤 해시 404를 자동 단언, 0/비0 종료코드
  - 문서에 요청/응답 예시(실제 출력)와 필드 설명 포함
- *Artifacts*
  - `scripts/verify-failed-tx.sh` (curl + 종료코드 단언)
  - `docs/api-failed-tx.md` (엔드포인트 1개 스펙 + 예시 — OpenAPI는 S04에서 통합)
- *Key Links*
  - `docker compose` 기동 + `docker compose run --rm seed` 전제 (KNOWLEDGE.md Lessons)
  - 시드 데이터에서 알려진 실패 tx_hash 1건을 픽스처로 명시

---

## Slice 수용 (Complete 게이트)

- [ ] T01·T02·T03 must-haves 전부 충족
- [ ] `cargo clippy -- -D warnings` / `cargo fmt --check` 통과
- [ ] `verify-failed-tx.sh` green
- [ ] 신규 public 함수 `///` 존재, 프로덕션 `unwrap()` 0
- [ ] Complete 후: M001-ROADMAP.md에서 S01 체크, S02 `[sketch]` 해제·태스크 분해,
      새 Lesson은 KNOWLEDGE.md 반영 (GSD-2 Reassess)

비고: 마이그레이션 없음(D004) · SQL/Rust 커밋 분리(KNOWLEDGE.md) · 프론트 계약 반영은
S01 범위 밖(별도 슬라이스에서 `web/src/api` 동기화).
