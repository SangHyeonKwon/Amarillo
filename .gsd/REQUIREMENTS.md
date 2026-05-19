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

## M003 — Actionable Alerts (출하 정의) `[sketch]`

> "실패 패턴 구독 → 웹훅으로 푸시. 온체인 × 비공개 데이터 조인 예시 1건."

(상세는 M001/M002 완료 후 Reassess에서 확정)

## 공통 비기능 요건

- CLAUDE.md 절대 규칙 준수 (no `unwrap()` in prod, parameterized SQL, 마이그레이션 경유 등 — KNOWLEDGE.md)
- 모든 신규 public 함수 `///` doc, 신규 마이그레이션 멱등
- 신규 엔드포인트마다 통합 검증(curl 또는 sqlx 통합 테스트)
