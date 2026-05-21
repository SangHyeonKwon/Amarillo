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

## M003 — Actionable Alerts + On-chain × Off-chain Join  ✅ SHIPPED → M003-SUMMARY.md
출하 정의: REQUIREMENTS.md#M003 (S08 ∧ S09). 수용 기준 전 항목 ✅, 최종 게이트
green (fmt clean · clippy --workspace 0 · `-p indexer` 36 · `-p db --lib` 14 ·
`-p db --ignored` 13 · verify 3종 ALL PASS · web typecheck/test 17 / build OK).

- [x] **S08 — 실패 패턴 구독 + 웹훅 전송** `[edge: untapped]` · risk: med · **DONE** → S08-SUMMARY.md
  - 마이그레이션(alert_subscription/alert_delivery) + 매칭/멱등 쿼리(anti-join)
    + outbox 디스패처(`indexer --dispatch-alerts`, SSRF 가드 `db::validators` 공유 +
    HMAC-SHA256 서명) + REST CRUD (POST 201 secret-once / GET no-leak / DELETE soft).
    신규 의존성 5개(reqwest+hmac+sha2+url+getrandom) — 정직 deviation(D012 REALIZED).
  - 게이트: `cargo test -p indexer` 22/22 · `-p db --lib` 9/9 · `-p db --ignored`
    10/10 · `verify-alerts.sh` ALL PASS · clippy --workspace 0 · fmt clean
- [x] **S09 — 온체인 × 비공개 라벨 조인 (M003 출하 게이트)** `[edge: untapped]` · **DONE** → S09-SUMMARY.md
  - `contract_label` 멱등 마이그레이션·시드(Uniswap V3 + pool) + DB pivot 쿼리 +
    `GET /v1/analytics/failed-tx/by-label` + `verify-failed-tx-by-label.sh` + web
    "Failures by labeled contract" 카드. **D013** (유스케이스 = 컨트랙트 라벨).

## M004 — Diagnostic Depth (+ developer product surface)  ✅ SHIPPED → M004-SUMMARY.md
출하 정의: REQUIREMENTS.md#M004. "어디서/어떤 함수가/왜+어떻게 실패했는지를
단건 응답에 정확하게 + 개발자가 카피해 즉시 쓰는 표면까지(S13)."
페르소나 = dApp 개발자(D014). 응답 4축(`failed`/`root_cause`/`failing_function_decoded`/
`diagnosis`)이 한 호출에 + TS/Python 예시 + cookbook으로 *프로덕트 표면* 완성.
수용 기준 전 항목 ✅, 최종 게이트 green (fmt/clippy/indexer 36/db --lib 14/
db --ignored 22/verify 3종 ALL PASS/web 26/build 900/TS tsc/Python py_compile).

- [x] **S10 — 콜트리 루트코즈 어트리뷰션** `[edge: untapped]` · risk: med · **DONE** → S10-SUMMARY.md
  - `/v1/failed-tx/{tx_hash}` 응답에 `root_cause: TraceLog | null` 가산 (D004 일관)
  - DB `get_first_error_frame`(trace_id ASC LIMIT 1) + 통합테스트 2 + verify
    `root_cause` 의미 단언(trace_id 일치) + 프론트 강조 블록. 명시 `null`만 —
    silent default 금지(D014).
- [x] **S11 — selector → 함수명 + signature 디코딩** `[edge: weak-spot]` · risk: med · **DONE** → S11-SUMMARY.md
  - `function_signature(selector PK, name, signature, source?)` 멱등 시드 17건
    (ERC20 5 + Uniswap V3 SwapRouter 6 + Factory 1 + Pool 3 + WETH9 2)
  - `FailedTxDetail.failing_function_decoded: DecodedFunction | null` 가산
    (D004 일관, silent default 금지) + DB lookup + 핸들러 가산 + 통합테스트 4 +
    verify DECODED semantics + 프론트 KPI 갱신. **D015** (args 분리, 자기시드 정책).
- [x] **S12 — 카테고리 진단 메시지 + 추천 액션** `[edge: weak-spot]` · risk: low-med · **DONE** → S12-SUMMARY.md
  - `category_diagnosis(error_category PK, message, recommended_action?, source?)`
    멱등 + 6 카테고리 시드(UNKNOWN/INSUFFICIENT_BALANCE/SLIPPAGE_EXCEEDED/
    DEADLINE_EXPIRED/UNAUTHORIZED/TRANSFER_FAILED) 모두 source='builtin'
  - `FailedTxDetail.diagnosis: Diagnosis | null` 가산 (D004 일관, silent default 금지)
    + `ErrorCategory::as_wire()` public 메서드 + DB lookup + 핸들러 가산 + 통합테스트 3
    + verify DIAG semantics(시드된 카테고리는 non-null 의미 단언) + 프론트 강조 블록.
    **D016** (스코프: 메시지+액션, enum 세분화는 S12.1).
- [x] **S13 — 개발자 예시 클라이언트(TS+Python) + cookbook** `[edge: weak-spot]` · risk: low · **DONE** → S13-SUMMARY.md
  - `examples/typescript-client/` (fetch + node:crypto, 외부 의존 0, ambient.d.ts로
    npm 무도입) — `AmarilloClient` 전 엔드포인트 + `verifyAlertSignature` + 3 시나리오
  - `examples/python-client/` (urllib + hmac stdlib, 외부 의존 0) — 동일 3 시나리오
  - `docs/cookbook.md` — 3 시나리오에 curl + TS + Python 3중 예시 + "M004 in one paragraph"
  - README.md 갱신 — Failure Intelligence API 표 확장 + "Client examples & cookbook" 신설
  - **D017** (예시 = SDK 동일, 게시는 S13.1)

> M004 잔여 sketch(S11.1 / S12.1 / S13.1)와 단독 백로그(DNS-time SSRF / Pools·Traders
> 매핑)는 [`BACKLOG.md`](BACKLOG.md) — 가치·리스크·페르소나·사전조건·크기 정리됨.

## M005 — Bot Operator Persona  ✅ SHIPPED → M005-SUMMARY.md
출하 정의: REQUIREMENTS.md#M005. "봇 운영자가 자기 봇의 실패 *패턴*을 *임계율*
알림으로 받고, 자기 봇 라벨을 동적으로 등록·관리해 분리된 분석을 받는다."
페르소나 = **봇 운영자**(D018). 응답 4축(rate sub + admin API + by-label?owner +
cookbook)이 모두 박힘 — 새 페르소나 완결. 수용 기준 전 항목 ✅, 최종 게이트
green (fmt/clippy/indexer 36/db --lib 14/db --ignored 27/verify 3종 ALL PASS/
web 29/build 900/TS tsc/Python py_compile).

- [x] **S14 — 임계율 집계 알림** `[edge: untapped]` · risk: med · **DONE** → S14-SUMMARY.md
  - `alert_subscription` + 4 컬럼(`sub_type` / `threshold_count` / `threshold_window_secs` /
    `debounce_secs`) 가산 + CHECK 제약 + `alert_rate_dispatch` 테이블
  - `find_pending_rate_alert_matches`(시간 윈도우 + 디바운스 검증) + `record_rate_alert_dispatch`
    + `dispatch_rate_once` (별 페이로드, claim 불필요 — SQL 디바운스 race-safe)
  - `find_pending_alert_matches`에 `sub_type='per_event'` 필터 추가(rate가 per-event
    matcher에 잡히는 회귀 차단)
  - API POST body 확장(rate fields, 잘못된 조합 5종 400) + GET 표시 + verify 의미 단언
  - 프론트 /alerts: sub_type 라디오 + 조건부 rate 폼 + 목록 Mode 컬럼
  - **D018** (컬럼 가산 + sub_type 명시 + 디바운스 시간 기반 + race 시맨틱 정직)
- [x] **S15 — 봇 라벨 admin API + 봇 운영자 cookbook (M005 마감)** `[edge: weak-spot]` · risk: low · **DONE** → S15-SUMMARY.md
  - `upsert_contract_label` (INSERT … ON CONFLICT DO UPDATE RETURNING) + `POST /v1/contract-labels`
    UPSERT + `DELETE /v1/contract-labels/{address}` 멱등 (404 두 번째)
  - 통합테스트 2 (upsert overwrite / delete idempotency) + verify 6 신규 (POST/upsert/400/DELETE/404/bad)
  - `docs/api-failed-tx.md` admin endpoint 3 subsection + auth-or-the-lack-of-it (D008/D019)
  - `docs/cookbook.md` 4번째 시나리오 "Bot operator playbook" (4 step curl+TS+Python receiver)
  - **D019** (M005 마감, 봇 라벨 별 테이블 X, 인증 X 데모 스코프)

---

## M006 — Operator Auth  ✅ SHIPPED → M006-SUMMARY.md
출하 정의: REQUIREMENTS.md#M006. "amarillo의 모든 write/admin 엔드포인트가
API key 인증으로 보호되고, 모든 검증·예시·프론트 흐름이 인증된 호출로 동작한다."
페르소나 = **운영자**(D021). M005까지의 "데모 스코프 인증 미부착"(D008/D013/D019)
정직성을 *운영 게이트*로 마감. 결정 묶음 **(A) API key Bearer + (X) write/admin
만 보호 + (1) env 단일 키** — D021/D022/D023.

- [x] **S16 — 인증 미들웨어 + 보호 게이트** `[edge: untapped — 운영 게이트]` · risk: med · **DONE** → S16-SUMMARY.md
  - T01 인프라: `crates/api/src/auth.rs` (AdminAuth + `subtle::ConstantTimeEq`)
    + `ApiConfig.admin_api_key` 필수 + Debug 마스킹(database_url + admin_api_key)
    + `ApiError::Unauthorized` + `routes/state.rs` (ApiState + FromRef) + `.env.example`/
    `docker-compose.yml` env 박음. 단위테스트 13(config 6 + auth 7).
  - T02 게이트: 보호 라우트 5개 핸들러에 `_: AdminAuth` 첫 파라미터 부착
    (POST/DELETE `/v1/contract-labels`, POST/DELETE/rotate `/v1/alert-subscriptions`).
    `crates/api/src/lib.rs` 신설(통합테스트용 표준 패턴), 통합테스트 7(보호 5 + 비보호 2).
  - 게이트: fmt clean / clippy --workspace --all-targets -D warnings 0 / api 20 /
    indexer 36 / db --lib 17 / db --ignored 27 / decoder 18 / web typecheck/test 29/build OK.
  - **D021** (A+X+1 묶음) · **D022** (extractor 게이트, 컴파일 시점 회귀 차단) · **D023** (env 단일 키, 빈 거부 + 짧으면 WARN).
  - 정직한 한계: verify 2종(alerts / failed-tx-by-label) + examples + 프론트
    *깨짐* — S17/S18 명시적 의존. toolchain 회귀 lint 2건 인라인 fix(별 단위 후보).

- [x] **S17 — verify 스크립트 + examples + cookbook 인증** · risk: low · **DONE** → S17-SUMMARY.md
  - T01 verify 3종: `${AMARILLO_ADMIN_API_KEY}` env 강제(`:?required`) + 보호
    라우트(POST/DELETE/rotate)에 `Authorization: Bearer` 헤더 + 401 case 2건씩
    (alerts / by-label). verify-failed-tx.sh는 GET only — env 강제만.
  - T02 examples (방향 A 호환 우선): TS `new AmarilloClient(baseUrl, { apiKey? })`,
    Python `AmarilloClient(base_url, api_key=None)`. `_request` 헬퍼에 `auth`
    옵션, write 5개 메서드 자동 헤더 + 키 없으면 *클라이언트 측 throw/raise*
    (서버 401 받기 전 사전 차단). examples.py/.ts main이 `AMARILLO_ADMIN_API_KEY`
    env에서 키 읽고, 키 없으면 demo #2(alert subscription) skip 안내.
  - T03 cookbook + docs: 글로벌 Authentication note + 시나리오 2 curl create/rotate
    헤더 + 시나리오 4 step 1/2 curl·TS·Python 헤더 + 키 패턴 + 새 절 "If you
    forget the API key" (401 사례). `docs/api-failed-tx.md` 상단 `## Authentication`
    종합 섹션 신설 (env 정책 + 보호 표 5 + 401 + curl 정상/실패 + 회전 절차 +
    JWT/OAuth 미선택 이유). 기존 "Auth (or the lack of it)" → 위 섹션 링크.
  - 게이트: fmt clean / clippy 0 / api 20 / indexer 36 / db --lib 17 / db --ignored 27 /
    decoder 18 / verify 3종 ALL PASS (포트 3005, 키 export) / tsc clean / py_compile clean /
    web typecheck/test 29/29 무회귀.
  - 새 결정 없음 — D021/D022/D023 일관 적용.

- [x] **S18 — 프론트 `/alerts` + M006 마감** · risk: low · **DONE** → S18-SUMMARY.md
  - T01 Context + 컴포넌트 + Alerts 통합: `web/src/state/apiKey.tsx`(신설 — Provider +
    `useApiKey()` hook + `setAdminApiKey()` module slot sync), `web/src/components/
    ApiKeyInput.tsx`(신설 — `type="password"` 입력 + Apply/Clear, 활성 시 길이만
    표시), `web/src/App.tsx` Provider 래핑, `web/src/pages/Alerts.tsx` 상단 배치 +
    `writesDisabled` 3 mutation 버튼 + form 위 안내 박스.
  - T02 `client.ts` 자동 헤더 + 401 안내: 모듈 `let _apiKey: string | null` +
    `setAdminApiKey()` export, `apiPost`/`apiDelete` 자동 부착(`apiGet` 무부착 —
    D021/X 일관), `describeError(err)` helper로 401 → "키 입력 패널 유도" 메시지.
    `web/src/api/client.test.ts` 8 case (setAdminApiKey 정규화 + apiPost/apiDelete/
    apiGet 헤더 단언 + null 즉시 wipe).
  - T03 cookbook + S18-SUMMARY + DECISIONS D024 + M006-SUMMARY: cookbook 시나리오
    5 "From the /alerts page" 신설 (Apply → Create → Rotate/Deactivate → 401 복구
    + D024 정신), D024 결정 기록(세션 메모리 + module-mutable + Context sync, A안
    채택 이유), M006-SUMMARY 작성 (세 슬라이스 통합 + 세 페르소나 완결 + 핵심
    교훈 6 + 정직한 한계 7).
  - 게이트: fmt clean / clippy 0 / api 20/indexer 36/db lib 17/db ignored 27/decoder
    18 무회귀 / verify 3종 무회귀 자동 (서버/스크립트 0 변경) / tsc/py_compile 무회귀
    자동 / web typecheck clean + test **37/37**(8 신규 + 26 + 3) + build OK.
  - **D024** (세션 메모리 + module-mutable + Context sync, A안 채택).

> M006 출하 = S16 ∧ S17 ∧ S18. **세 슬라이스 모두 ✅ → M006 ✅ SHIPPED**.
> 세 페르소나(dApp 개발자 / 봇 운영자 / 운영자) 완결. 자세한 잔여·후속 후보는
> `M006-SUMMARY.md` 또는 `BACKLOG.md` 우선순위 표.

---

## 백로그

미완료 항목은 [`BACKLOG.md`](BACKLOG.md)에 통합(가치/리스크/페르소나/사전조건/크기
+ 우선순위 표). 마일스톤 분기 시 그쪽을 *시드*로 사용.

완료 백로그 (한 줄 압축):

| 항목 | 결과 |
|------|------|
| TEST-HARNESS — db cargo 통합테스트 하니스 | D007 RESOLVED · STH-SUMMARY.md |
| S04 하드닝 (리뷰 L1–L3) — 명시컬럼 / tx_hash 400 / call_tree 상한 | S04에서 해소 · S04-SUMMARY.md |
| HARDEN — follow cycle cap + ctrl_c + outbox claim + bounded + mock receiver | HARDEN-SUMMARY.md |
| HARDEN2 — last_error URL 마스킹 + signing_secret 회전 API | HARDEN2-SUMMARY.md |
| FE-WIRE — Failed Tx 페이지 3섹션 재결선(timeseries/list/inspect) | FE-WIRE-SUMMARY.md |
| FE-WIRE2 — `/alerts` 페이지(목록·생성·회전·비활성·시크릿 1회 모달) | FE-WIRE2-SUMMARY.md |
| S11.1 — ABI args 디코딩 + root_cause.input 디코드 (M004 깊이 가산) | D025/D026/D027 · S11p1-SUMMARY.md |
| S12.1 — ErrorCategory enum 세분화 v2 (M004 정밀도 가산) | D028/D029/D030 · S12p1-SUMMARY.md |

## Reassess 규칙 (GSD-2)
각 슬라이스 Complete 후 이 ROADMAP 갱신: 다음 슬라이스 `[sketch]` 해제·태스크 분해,
새 Lesson은 KNOWLEDGE.md, 방향 변경은 DECISIONS.md. **M001 ~ M006 모두 출하 완료** —
제품의 세 페르소나(dApp 개발자 / 봇 운영자 / 운영자) 완결. 후속 마일스톤(M007 등)
또는 단독 슬라이스 분해는 사용자 지시 시 (GSD-2: 출하 전 분해 금지 원칙 일관).
후보는 [`BACKLOG.md`](BACKLOG.md) 우선순위 표 + M006-SUMMARY.md "잔여" 섹션 참조.
