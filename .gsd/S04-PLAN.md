# S04 — 임베드 가능화 + 하드닝 (M001 마무리) · PLAN

Slice 목표: M001 수용 기준(REQUIREMENTS.md#M001) 전체 충족 + 리뷰 백로그 L1–L3 해소 +
외부 임베드 가능 상태로 마감 → Milestone Validate.

엣지: `[edge: untapped]` — "임베드 가능한 실패 인텔리전스 API"는 Dune이 제공하지 않는 형태.

전제: S01–S03 완료. 마이그레이션 없음. OpenAPI 프레임워크 미도입(D008).
태스크: T01 → T02 → T03.

---

## T01 — 하드닝 (리뷰 L1–L3)

**Must-haves**
- *Truths*
  - L1: 실패-tx 관련 조회는 `SELECT *` 대신 **명시 컬럼**. 응답 JSON 형태 불변(검증 스크립트 green 유지)
  - L2: 잘못된 `tx_hash` 형식(`^0x[0-9a-fA-F]{64}$` 불만족) → **400**(404 아님).
    유효하나 미존재 → 여전히 404
  - L3: `call_tree`가 비정상적으로 크면 상한(예: 2000 프레임) + 응답에 `truncated` 표시,
    또는 명시적 한계 문서화 — 택1, 테스트 동반
- *Artifacts*: `crates/db/src/queries.rs`(명시 컬럼), `crates/api/src/routes/failed_tx.rs`
  (tx_hash 검증 헬퍼), 관련 통합/스크립트 테스트
- *Key Links*: 기존 `ApiError::BadRequest`, S02 검증 경계 패턴
- *검증*: `cargo test -p db -- --ignored` + verify 스크립트에 L2 케이스 추가, 전부 green

## T02 — 임베드 가능화 (API 레퍼런스 + 원커맨드)

**Must-haves**
- *Truths*
  - `docs/api-failed-tx.md`가 4개 엔드포인트(단건/목록/시계열 + 에러규약)를 단일 레퍼런스로
    완결 — 요청/응답/상태코드/예시 일관
  - `scripts/verify-failed-tx.sh`가 곧 실행 가능한 스모크 겸 예제(원커맨드)임을 문서화
- *Artifacts*: `docs/api-failed-tx.md` 통합·정리, README 또는 docs 인덱스에서 링크
- *비범위*: `utoipa` 등 OpenAPI 프레임워크(D008, `[sketch]`)

## T03 — Milestone Validate (M001 마감)

**Must-haves**
- *Truths*
  - REQUIREMENTS.md#M001 수용 기준 항목별 ✅ 체크 (단건/목록/시계열/계약/재현 스크립트·문서)
  - 전체 게이트: `cargo clippy -p db -p api --tests -- -D warnings`, `cargo fmt --check`,
    `cargo test -p db -- --ignored`, `verify-failed-tx.sh` 모두 green
- *Artifacts*: `.gsd/S04-SUMMARY.md`, `.gsd/M001-SUMMARY.md`(마일스톤 종결),
  ROADMAP M001 `[x]`
- *Reassess*: M001 출하 확정 후에야 M002(`[sketch]`) 분해 착수 가능(GSD-2)

---

## Slice 수용 (Complete = M001 출하)
- [ ] T01·T02·T03 must-haves 충족, L1–L3 백로그 해소
- [ ] 전체 게이트 green, M001 수용 기준 전체 ✅
- [ ] M001-SUMMARY.md 작성, ROADMAP M001 마감, M002 분해 가능 상태로 전환
