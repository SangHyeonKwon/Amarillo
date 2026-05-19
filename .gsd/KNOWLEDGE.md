# KNOWLEDGE

GSD-2: Rules(불변) + Patterns/Lessons(누적). 실행 전 반드시 읽는다.

## Rules — CLAUDE.md 절대 규칙 (위반 금지)

- 프로덕션 코드 `unwrap()` 금지 → `?` 또는 명시적 에러 처리. 테스트만 허용
- 유저 입력 raw SQL 금지 → 반드시 `sqlx::query!`/`query_as!` 파라미터화
- 스키마 변경은 `migrations/` 경유만. SQL 멱등 (`IF NOT EXISTS`, `ON CONFLICT`)
- `main` 직접 푸시 금지 → feature 브랜치 + PR. 커밋은 사용자 요청 시에만
- 모든 public 함수 `///` doc. 크레이트당 하나의 `thiserror` Error, 바이너리는 `anyhow`
- 커밋 단위: SQL과 Rust를 같은 커밋에 섞지 않음

## Patterns — 기존 코드에서 그대로 따를 형태

- **읽기 쿼리**: `crates/db/src/queries.rs`의 `list_swap_events` / `get_failed_tx_analysis`
  패턴 (sqlx `query_as::<_, Model>`, `($1::TEXT IS NULL OR col=$1)` 옵션 필터, LIMIT/OFFSET)
- **모델**: `crates/db/src/models.rs` — `sqlx::FromRow + serde::Serialize`. 신규 응답 모델도 여기에
- **API 라우트**: `crates/api/src/routes/<name>.rs` 핸들러 + `routes/mod.rs`의 `v1_router()`에 라우트 등록
- **에러 매핑**: `crates/api/src/error.rs` — `DbError::NotFound → ApiError::NotFound(404)` 이미 존재
- **응답 봉투**: `crates/api/src/response.rs` — 단건 `ApiResponse{data}`, 목록 `PaginatedResponse`
- **프론트 계약**: 변경 시 `web/src/api/types.ts` + `web/src/api/contract.ts`(런타임 파싱) 동기화

## Lessons — 이 코드베이스의 함정 (실측 확인됨)

- **인덱서는 pool/token/user_profile을 안 채운다**. `crates/indexer/src/worker.rs`는 block/tx/
  events/trace/failed만 INSERT. `pool`/`token`/`user_profile`은 seed SQL(`sql/dml/001_seed_data.sql`)
  로만 채워짐. FK는 `migrations/20240102000001_relax_fk_and_checkpoint.sql`로 완화돼 있어 이벤트
  INSERT가 깨지지 않음. → 실패tx 진단은 이 함정과 무관(자체 데이터)하지만 인지 필요.
- **docker compose는 인덱서를 안 띄움** (postgres+api+web만). 실시간 작업(M002)은 인덱서 실행
  경로/배포를 새로 설계해야 함.
- **`error_category`는 PascalCase로 직렬화**될 수 있음(`"Unknown"`). 프론트 `contract.ts`가
  정규화 중. 신규 API도 동일 enum 직렬화 규약 유지.
- **Dockerfile 빌더 이미지**: `rust:1.90-slim-bookworm` (1.86은 `home@0.5.12` MSRV 1.88로 빌드 실패).
  이 변경 현재 uncommitted.
- **`failed_transaction` / `trace_log` 스키마 (models.rs 확인)**:
  - `failed_transaction(tx_hash PK, error_category, revert_reason?, failing_function?, gas_used, timestamp)`
  - `trace_log(tx_hash, call_depth, call_type, from_addr, to_addr?, value, gas_used, input?, output?, error?, trace_id)`
  - 기존 집계 뷰 `vw_failed_tx_analysis`는 카테고리별 COUNT만 — **개별 진단 불가** (S01이 메우는 갭)
- **[S01 실측] 제품 가설 검증됨**: 시드 `failed_transaction`은 분류 안 됨(`error_category=Unknown`,
  `revert_reason=null` — 시드는 정적, 라이브 classifier 미경유). 그런데 단건 진단 엔드포인트가
  반환한 `trace_log` 프레임엔 `error="Too little received"`(슬리피지)가 찍혀 있음. 즉
  **단건 진단 = 집계보다 엄격히 더 많은 정보**. D002 가설의 첫 실증. (Lesson: 추후 슬라이스에서
  trace.error를 분류 입력으로 재활용하면 시드/미분류 데이터도 진단 가능 — S02 이후 후보)
- **[S01] 신규 엔드포인트 라이브 검증법**: docker postgres(:5432)에 로컬 api를 `API_PORT=3001`로
  붙여 검증. `scripts/verify-failed-tx.sh`가 그 절차를 캡슐화 (docker api 컨테이너는 구 이미지라
  코드 변경분은 로컬 바이너리로 확인). 신규 슬라이스도 동일 패턴 재사용.
- **[S01 리뷰] 트레이스 선형 순서 = `trace_id ASC` (불변식)**: `trace_log`는 인덱서가 콜트리를
  pre-order DFS로 평탄화하며 삽입한 순서를 `trace_id`(BIGSERIAL)에 보존한다. trace 관련 신규
  쿼리는 **반드시 `ORDER BY trace_id ASC`**. `call_depth` 우선 정렬 금지(형제 서브트리가 섞여
  트리 복원 불가). 최초 S01 구현이 이 실수를 했고 리뷰에서 교정(H1).
- **[S01 리뷰] 검증은 shape가 아니라 semantics를 단언하라**: "필드 존재"만 보면 정렬·불변식
  버그를 못 잡는다(H1이 스모크를 통과한 이유). 가능한 불변식(정렬, 카운트 일치 등)을 명시 단언.
  `verify-failed-tx.sh`에 node 기반 순서 단언 추가됨.
- **[STH] DB 통합테스트 관례 확립**: PG 필요한 테스트 = `crates/db/tests/*.rs` 별도 크레이트,
  `#[tokio::test] #[ignore = "..."]`, 실행 `cargo test -p db -- --ignored`. 통합테스트 크레이트는
  `db`의 일반 deps 미상속 → `db/Cargo.toml [dev-dependencies]`에 `tokio` 필요. 타입명 노출
  피하려 `db::create_pool` 인라인(=sqlx dev-dep 불요). `DATABASE_URL` 미설정 시 docker 기본값.
  신규 db 쿼리는 이 패턴으로 불변식 테스트 동반. (M1 종결, D007 RESOLVED)
- **[S02] 검증 경계 & 가산적 진화 (Pattern)**: 신뢰불가 입력의 파싱·거부는 API 엣지 책임.
  잘못된 입력 = **400 BadRequest**, 정상요청·리소스없음 = **404**. DB 계층은 이미 타입된
  값만 받는다. 응답 계약은 기존 것을 변형하지 말고 **새 봉투 추가**
  (`TotalPaginatedResponse`, 기존 `PaginatedResponse` 불변; D005 REALIZED) →
  프론트 `contract.ts`는 새 파서만 추가.
- **[S02] str→enum 단일 출처**: `impl FromStr for ErrorCategory`(models.rs, SCREAMING_SNAKE)가
  쿼리파라미터 파싱의 단일 출처. 신규 엔드포인트의 카테고리 입력은 재사용(중복 매핑 금지).
  주의 비대칭: serde **직렬화**는 변형명(`"Unknown"`), 입력 **파싱**은 와이어명(`"UNKNOWN"`).
- **[S03] 인젝션 안전 동적 집계 (Rule)**: `date_trunc` 같은 동적 SQL 조각은 절대 문자열
  보간 금지. 이중 방어: ① 입력을 닫힌 enum 화이트리스트(`TimeBucket`)로만 받아 고정
  리터럴 변환, ② 그 리터럴조차 `date_trunc($1, ...)` **바인딩 파라미터**로 전달.
  표현식 중복 회피는 `GROUP BY 1,2 / ORDER BY 1,2`(ordinal). 새 동적 집계는 이 규칙 준수.
- **[S04] 마일스톤 검증 = 게이트 전체 재실행 (Rule)**: 슬라이스별 green이 누적됐어도
  마일스톤 마감 시 clippy+fmt+통합테스트+HTTP 스크립트를 **새로 한 번 더** 돌린다.
  근거: 작업 중 환경 드리프트(Docker 다운으로 통합테스트 전멸 경험) — 누적 green은
  과거 시점 증빙일 뿐. 코드 무변경이어도 재실행이 출하 증빙.
- **[S04] 무제한 컬렉션엔 상한+신호 (Pattern)**: 외부 노출 배열은 `LIMIT N+1` fetch로
  잘림 감지 후 N개로 자르고 `*_truncated: bool`로 부분응답 신호. 무제한 = 임베드 API의
  실제 DoS/성능 위험.
