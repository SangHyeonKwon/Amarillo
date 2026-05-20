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

## 공통 비기능 요건

- CLAUDE.md 절대 규칙 준수 (no `unwrap()` in prod, parameterized SQL, 마이그레이션 경유 등 — KNOWLEDGE.md)
- 모든 신규 public 함수 `///` doc, 신규 마이그레이션 멱등
- 신규 엔드포인트마다 통합 검증(curl 또는 sqlx 통합 테스트)
