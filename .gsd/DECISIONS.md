# DECISIONS

확정된 방향 결정 (GSD-2: memory-backed). 번복 시 새 항목 추가, 옛 항목 `SUPERSEDED` 표기.

## D001 — 일반 대시보드 포기, 백엔드 재개
- **결정**: Overview/Pools/Top Traders 같은 일반 분석은 더 만들지 않는다. 이전 "백엔드 동결" 지시는
  목적이 '제품'으로 확정되며 **무효**. 엣지는 백엔드에 있으므로 백엔드를 다시 연다.
- **이유**: 일반 분석은 Dune이 압도. 거기 투자할수록 엣지가 깎임 (PROJECT.md).

## D002 — 제품 가설: Transaction Failure Intelligence
- **결정**: 제품의 축을 "실패 트랜잭션 진단/인텔리전스"로 고정.
- **이유**: 차별 자산(`crates/decoder/src/trace.rs` + `classifier.rs`)이 이미 절반 존재. Dune이
  out-of-box로 제공 안 함. 실수요 존재.

## D003 — 스코프 동결: Ethereum + Uniswap V3
- **결정**: 체인/프로토콜 확장 금지. 깊이(실시간·진단·정합성) 우선.
- **이유**: 폭은 Dune이 이김. 좁고 깊게가 유일한 엣지 경로.

## D004 — 콜트리는 우선 "평탄화 프레임"으로 반환
- **결정**: `/v1/failed-tx/{hash}`의 call tree는 `trace_log`에 저장된 평탄 프레임을
  `call_depth` 순서로 반환. 중첩(JSON 트리) 재구성은 후속 `[sketch]`.
- **이유**: `trace_log`가 이미 평탄 저장 (`decoder::trace::flatten_call_frame`). 최소 변경으로
  최대 가치. 중첩은 소비자 요구 확인 후.

## D005 — 페이지네이션에 정확한 total 추가 (신규 엔드포인트 한정)
- **결정**: 신규 실패 목록 API는 `COUNT(*)` 기반 `total`을 응답에 포함. 기존 엔드포인트는
  계약 호환 위해 변경하지 않음.
- **이유**: 임베드형 제품엔 total이 필수. 기존 `PaginatedResponse`는 count(현재 페이지 길이)만
  제공 — 한계를 신규에서만 보완 (web/README의 "explicit total" 후보와 일치).
- **실현 (S02, REALIZED)**: `response.rs`에 `TotalPaginatedResponse<T>` + `PaginationMeta`
  추가, `GET /v1/failed-tx`가 사용. 기존 `PaginatedResponse`/`PaginationInfo` 불변 유지.
  라이브 검증 `total=3, returned=2`(limit=2)로 LIMIT 독립성 확인. → S02-SUMMARY.md

## D006 — `.gsd/`는 계획 문서로만, gsd-2 CLU/DB 미사용
- **결정**: gsd.db/STATE.md 등 런타임 산출물은 만들지 않음. PLAN/ROADMAP/SPEC markdown만 유지.
- **이유**: gsd-2 런타임은 외부 도구. 우리는 방법론·문서 구조만 차용.

## D007 — S01 검증: cargo 통합테스트 → 스크립트+의미단언으로 대체, 정식 테스트 이월
- **결정**: S01-PLAN T01이 명시한 `cargo test -p db -- --ignored` 통합테스트는 S01에서
  구축하지 않는다. 대신 `scripts/verify-failed-tx.sh`(라이브 curl + node 순서 단언)로 수용.
  정식 cargo 통합테스트 하니스(db dev-deps/runtime 구성)는 **독립 1유닛으로 이월** →
  M001-ROADMAP 백로그 `S0x-TEST-HARNESS`로 추적.
- **이유**: 테스트 하니스 구축은 그 자체로 한 유닛(Cargo dev-deps, tokio runtime, 시드 의존).
  S01에 끼워넣으면 "태스크는 컨텍스트 1개" 규칙 위반. 회귀가드는 스크립트 의미단언으로 즉시 확보.
- **리스크**: 스크립트는 CI에서 docker+seed 전제 필요(순수 `cargo test`보다 이식성↓). 이월 유닛에서 해소.
- **연관**: 리뷰 H1(트레이스 정렬 버그)이 이 갭으로 새어나갔음 → 스크립트에 순서 단언 추가로 보강.
- **해소 (STH, RESOLVED)**: `crates/db/tests/failed_tx.rs` 통합테스트 4건 구축
  (`#[ignore]`, `cargo test -p db -- --ignored`). H1 불변식이 실행 가능한 회귀 테스트로 박힘.
  M1 종결. 쉘 스크립트는 HTTP 레벨 상호보완으로 유지. → STH-SUMMARY.md

## D008 — API 레퍼런스는 손작성 우선, OpenAPI 프레임워크 보류
- **결정**: S04의 "임베드 가능화"는 `utoipa` 등 OpenAPI 생성 프레임워크를 도입하지 않고
  손작성 `docs/api-failed-tx.md` 통합 + curl 원커맨드로 충족. 머신리더블 `openapi.json`은
  필요 시 최소 손작성. 프레임워크 도입은 `[sketch]`로 보류.
- **이유**: 프레임워크는 의존성·매크로 침투(전 핸들러 어노테이션)로 그 자체가 한 유닛 이상.
  현재 엔드포인트 4개 규모엔 손작성이 ROI 우위. 규모 커지면 재검토.
- **트레이드오프**: 손작성은 표류 위험 → 검증 스크립트/통합테스트가 실계약을 강제하므로 완화.

## D009 — S05 실시간: poll + confirmations lag, eth_subscribe·깊은 reorg 보류
- **결정**: S05 체인 헤드 팔로워는 ① `eth_subscribe`가 아니라 **polling**(get_block_number),
  ② 헤드가 아니라 **head − N confirmations**까지만 인덱싱(얕은 reorg 노출 최소화).
  `eth_subscribe`는 S07(하드닝), **깊은 reorg 감지·정정은 S06**으로 분리.
- **이유**: polling은 RPC 호환성↑·구현 단순(연속 루프 학습에 집중). confirmations lag는
  S06 전까지의 실용적 reorg 완화. 한 슬라이스 = 한 관심사(GSD-2).
- **트레이드오프**: confirmations만큼 지연 발생(수 초~분) — REQUIREMENTS#M002 "수 초 내"는
  S06/S07 합산으로 충족. 단순/안전 우선.
- **검증 제약**: 실시간 follow는 `RPC_URL` 필요(CI/환경 부재 가능) → 순수 결정 로직
  `next_target()`를 분리해 단위테스트(RPC 불요), 라이브 follow는 수동 스모크·문서.
