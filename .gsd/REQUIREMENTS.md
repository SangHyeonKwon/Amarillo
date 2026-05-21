# REQUIREMENTS

GSD-2: "shippable"의 정의와 수용 기준. Milestone마다 무엇이 되면 출하인지.

## 페르소나 & 잡(Job)

- **댑 개발자**: "내 컨트랙트로 들어온 tx들이 왜 실패하지?" → 카테고리별/시간별 조회 + 단건 진단
- **지갑/CS**: "유저가 준 이 tx 해시, 사람이 읽을 사유로 설명해줘" → `/{tx_hash}` 단건
- **봇 운영자**: "revert율이 급증하면 즉시 알려줘" → 실시간 + 알림 (후속 마일스톤)

## M001 — Failure Intelligence Core API (출하 정의)

> "인덱싱된 임의의 실패 tx를 조회·진단할 수 있고, 외부 제품이 임베드할 수 있다."

수용 기준 (mechanically checkable):

- `GET /v1/failed-tx/{tx_hash}` → 디코딩된 revert 사유 + `error_category` + 평탄화된 콜트리 반환.
  미존재 해시 → 404 `{ "error": ... }`
- `GET /v1/failed-tx?category=&from=&to=&limit=&offset=` → 필터·페이지네이션된 실패 목록,
  응답에 **정확한 total** 포함 (기존 뷰 API의 한계 보완)
- `GET /v1/analytics/failed-tx/timeseries?interval=&from=&to=` → 카테고리별 실패 추이
- 위 3개가 기존 `ApiResponse`/`PaginatedResponse` 계약·에러 규약을 따름
- `curl` 재현 스크립트 + 엔드포인트 문서가 저장소에 존재 (임베드 가능 증빙)

## M002 — Real-time Failure Pipeline (출하 정의)

> "새 블록의 실패가 수 초 내 조회 가능하고, reorg에도 데이터가 정합적이다."

- 인덱서가 체인 헤드를 따라가며 신규 블록 실패를 자동 분류·저장 (연속 루프)
- reorg 발생 시 영향 블록 데이터 정정, 멱등성 유지, 중복/유령 행 없음

## M003 — Actionable Alerts (출하 정의)

> "실패 패턴 구독 → 웹훅으로 푸시. 온체인 × 비공개 데이터 조인 예시 1건."

M001·M002 출하로 확정(Reassess). M003 = **S08(구독+웹훅) + S09(조인 예시)**.

**S08 — 실패 패턴 구독 + 웹훅 전송** 수용 기준 (mechanically checkable):
- `POST /v1/alert-subscriptions {webhook_url, error_category?, to_addr?}` → 생성.
  안전하지 않은 `webhook_url`(비-https / loopback / RFC1918 / link-local /
  메타데이터 IP)은 **400**. `GET /v1/alert-subscriptions`, `DELETE /{id}` 동작.
  기존 `ApiResponse`/에러 규약 준수.
- 디스패처가 신규 매칭 실패 tx에 대해 구독 `webhook_url`로 **HMAC-SHA256 서명된**
  POST를 **정확히 1회**(멱등 — `alert_delivery`) 전송. 전송 실패는 재시도·기록.
- 마이그레이션 멱등, 신규 public `///`, SSRF 가드·서명·매칭 단위테스트 +
  매칭 쿼리 통합테스트 + `scripts/verify-alerts.sh` 재현 스크립트/문서.

**S09 — 온체인 × 비공개 데이터 조인 예시** `[sketch]`: 실패 인텔리전스 × 비공개
(시드/오프체인 라벨 등) 조인 1건. 가장 방어 가능한 해자 — 소비 유스케이스
확정 후 S08 출하 시점에 정제(별도 슬라이스). **M003 출하 = S08 ∧ S09.**

## M004 — Diagnostic Depth (출하 정의)

> "임의의 실패 tx에 대해 *어디서/어떤 함수가/왜* 실패했는지를 단건 호출에 정확하게."

페르소나 = **dApp 개발자**(D014). M001~M003가 데이터·실시간·알림·라벨 조인을
박았다면, M004는 *진단 그 자체의 품질*. 새 분석 엔드포인트 추가가 아니라 기존
`/v1/failed-tx/{tx_hash}` 응답이 누적적으로 더 똑똑해진다.

M003 분해 패턴 일관: **M004 = S10 ∧ S11 ∧ S12** (S13 SDK는 `[sketch]`).

**S10 — 콜트리 루트코즈 어트리뷰션** 수용 기준 (mechanically checkable):
- `GET /v1/failed-tx/{tx_hash}` 응답에 `root_cause: TraceFrame | null` 필드 노출.
  `trace_log`에서 `error IS NOT NULL`인 가장 빠른(=trace_id ASC) 노드 1건
  (depth, call_type, addresses, selector, error). 미존재 시 명시 `null`
  (silent failure 금지).
- 기존 `call_tree` 배열 + `call_tree_truncated` 계약 불변(D004 일관). 새 필드만 가산.
- 신규 DB 쿼리 통합테스트(첫 error frame을 정확히 잡고, error 없는 시나리오는
  `None`), `scripts/verify-failed-tx.sh`에 `root_cause` 의미 단언 추가, 프론트
  단건 화면에 "Root cause" 카드.

**S11 — failing_function selector → 함수명 + decoded args** `[sketch]`
**S12 — 카테고리 세분화 v2 + 진단 메시지/추천액션** `[sketch]`
**S13 — 개발자 SDK/문서 (TS/Python 미니멈 클라이언트 + cookbook)** `[sketch]`

## M005 — Bot Operator Persona (출하 정의)

> "봇 운영자가 자기 봇의 실패 *패턴*을 *임계율* 알림으로 받는다 — 건별 노이즈 없이."

페르소나 = **봇 운영자**(D018). M001~M004는 dApp 개발자 페르소나 직격이었다면,
M005는 새 페르소나 진입 — *건별* 알림(S08)은 봇 운영자에게 노이즈가 됨. 봇은
실패가 *간헐적으로* 발생하면 정상 운영의 일부지만 *급증*하면 봇이 망가졌다는
신호. M005는 그 비정상 패턴을 *임계율*로 잡아 알림.

**S14 — 임계율 집계 알림** 수용 기준 (mechanically checkable):
- `alert_subscription`에 `sub_type` ('per_event' | 'rate_threshold') 컬럼 가산 +
  `threshold_count`/`threshold_window_secs`/`debounce_secs` (rate 모드 필수).
  기존 행은 default `sub_type='per_event'` — 완전 호환(silent default 금지 정신
  일관, 명시 default).
- `POST /v1/alert-subscriptions` body에 rate 필드 받기. 잘못된 조합(예: rate인데
  threshold 누락)은 **400**.
- 디스패처: rate 모드 sub은 시간 윈도우 내 매칭 실패 count >= threshold면
  1회 webhook 발송 + `debounce_secs` 동안 같은 sub은 무시.
- `scripts/verify-alerts.sh`에 rate sub 시드 + 의미 단언 추가, 프론트 `/alerts`
  페이지에 rate 설정 폼 + rate 모드 시각화.

**S15 — 봇 라벨 admin API + 봇 운영자 cookbook (M005 마감 슬라이스)** 수용 기준:
- `POST /v1/contract-labels {address, label, owner_id?}` → 201로 라벨 등록.
  잘못된 주소(0x+40hex 아님) → 400. 인증 미부착(D008/D019 데모 스코프).
- `DELETE /v1/contract-labels/{address}` → 미존재면 404, 존재면 204 (멱등).
- `docs/cookbook.md`에 시나리오 4 추가: "봇 운영자 흐름" — 라벨 등록 → rate sub
  생성(자기 봇 to_addr) → 발송 → by-label로 자기 봇 실패 분포 확인. curl + TS + Python.
- `scripts/verify-failed-tx-by-label.sh`에 POST/DELETE round-trip 단언 추가.
- (S16 흡수 — 별 슬라이스 미생성, M005 마감 묶음.)

## M006 — Operator Auth (출하 정의)

> "amarillo의 모든 write/admin 엔드포인트가 API key 인증으로 보호되고, 모든
> 검증·예시·프론트 흐름이 인증된 호출로 동작한다."

페르소나 = **운영자**(D021). M001~M005는 "데모 스코프 인증 미부착"(D008/D013/D019)
정직성으로 마감했으나 외부 노출 시 *별 마일스톤 가치*. 본 마일스톤은 인증
미들웨어를 *별 슬라이스로 분해*해 단일 PR 부담을 분산 — write/admin만 보호
(GET은 임베드성 보존, X), env 단일 키(scope/multi-key X, 1), API key Bearer
헤더(JWT/OAuth X, A).

M005 분해 패턴 일관: **M006 = S16 ∧ S17 ∧ S18.**

**S16 — 인증 미들웨어 + 보호 게이트** 수용 기준 (mechanically checkable):
- `AMARILLO_ADMIN_API_KEY` env 미설정 또는 빈 문자열 → 서버 **부팅 실패**
  (silent default 금지, D004 정신). 32바이트 이상(hex 64자) 권고(부팅 시 길이
  부족 WARN 로깅, 거부 X — 운영 유연성).
- axum extractor(`AdminAuth` from_request_parts) — `Authorization: Bearer <key>`
  헤더 누락/형식 오류/키 불일치 모두 **401** `{"error":"unauthorized"}`. 헤더는
  있으나 키 불일치는 *동일 401* (info-leak 방지 — 키 존재 여부 노출 X).
- 키 비교는 **상수시간** (`subtle::ConstantTimeEq` 또는 자체 ct_eq) — timing
  attack 방어.
- 보호 대상 (X 정책): `POST /v1/contract-labels`, `DELETE /v1/contract-labels/{address}`,
  `POST /v1/alert-subscriptions`, `DELETE /v1/alert-subscriptions/{id}`,
  `POST /v1/alert-subscriptions/{id}/rotate-secret`. GET 전부 비보호.
- 키는 **로그에 절대 미노출** (`ApiConfig` Debug에서 마스킹 — HARDEN2 정신).
- `ApiError::Unauthorized` variant 추가 + 통합테스트(헤더 없음 / 잘못된 헤더 /
  잘못된 키 → 모두 401, 올바른 키 → 기존 200/201/204).

**S17 — verify 스크립트 3종 + examples 클라이언트(TS/Python) + cookbook 인증** 수용 기준:
- `scripts/verify-{failed-tx,alerts,failed-tx-by-label}.sh` 모두 `${AMARILLO_ADMIN_API_KEY}`
  env 읽기 — 미설정 시 **즉시 실패**(silent skip 금지). 보호 라우트 호출에
  `Authorization: Bearer ${key}` 헤더 추가, 잘못된 키 401 시나리오 1건 추가.
- `examples/typescript-client/client.ts`: `new AmarilloClient({baseUrl, apiKey?})`
  로 옵션 가산 — `apiKey` 있으면 모든 *write* 요청에 `Authorization: Bearer`
  헤더 자동 부착, GET은 없으면 미부착(임베드성). `tsc --noEmit` clean.
- `examples/python-client/client.py`: 동일 — `AmarilloClient(base_url, api_key=None)`,
  `py_compile` clean.
- `docs/cookbook.md` 4 시나리오 모두 인증 헤더 명시 — 봇 운영자 시나리오 step 1·2
  (라벨 등록 / sub 생성)에 `Authorization: Bearer` 추가, 401 사례 1건 명시.
- `docs/api-failed-tx.md`에 "Authentication" 섹션 추가 — env 키 정책 + 보호
  대상 표 + curl 예시.

**S18 — 프론트 `/alerts` 페이지 + M006 마감** 수용 기준:
- `web/`의 `/alerts` 페이지: API 호출 시 인증 헤더 부착 — `NEXT_PUBLIC_AMARILLO_ADMIN_API_KEY`
  env 또는 페이지 상단 키 입력 UI(*세션 메모리만*, localStorage 미저장 — XSS
  표면 최소화). 키 미설정 시 *생성/회전/비활성* 버튼 비활성 + 안내 배너.
- 401 응답 처리 — 명확한 에러 메시지("Unauthorized — check API key").
- `web` typecheck/test/build clean. M006 마감 cookbook 4번째 시나리오 step 1·2의
  curl/TS/Python 모두 인증 헤더 시연.

## 공통 비기능 요건

- CLAUDE.md 절대 규칙 준수 (no `unwrap()` in prod, parameterized SQL, 마이그레이션 경유 등 — KNOWLEDGE.md)
- 모든 신규 public 함수 `///` doc, 신규 마이그레이션 멱등
- 신규 엔드포인트마다 통합 검증(curl 또는 sqlx 통합 테스트)
