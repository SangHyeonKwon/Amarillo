---
slice: FE-WIRE
title: web/ 대시보드 ↔ 신규 실패-인텔리전스 API 결선
status: done
edge: weak-spot
milestone: M001 후속 (frontend consumption)
tasks: [T01, T02]
gate: pass             # typecheck clean · vitest 10/10 · build OK
migrations: none
decision: none new
artifacts:
  - web/src/api/types.ts           # FailedTransaction/TraceLog/FailedTxDetail/FailedTxTrendPoint/PaginationMeta/TotalPaginatedResponse/TimeBucket
  - web/src/api/contract.ts        # 신규 5 파서 + envelope 3개 + parsePaginationMeta/parseTotalPaginatedResponse
  - web/src/api/hooks.ts           # useFailedTxDetail / useFailedTxList / useFailedTxTimeseries
  - web/src/api/contract.test.ts   # 신규 4 케이스 (detail + list + timeseries + malformed throw)
  - web/src/pages/FailedTx.tsx     # Trend chart 교체 + Browse list 신규 + Tx inspection 신규
verification_constraint: "vitest 단위 + 빌드/typecheck로 계약 정합성·렌더 시그처 가드. 실제 데이터 표시는 docker postgres+api 대상 로컬 dev 서버에서 수동 클릭 스모크."
---

# FE-WIRE — 무엇이 실제로 일어났나

ROADMAP 백로그의 *FE-WIRE (R2)* — `web/`가 만들어진 뒤 S01~S04에서 추가된
실패-인텔리전스 API가 contract.ts에 안 박혀 있어 대시보드가 신규 엔드포인트를
못 보던 상태. 백엔드 무변경, 마이그레이션 0, 새 결정 0.

- **T01 (contract sync + 신규 hooks)** — types.ts에 신규 7형 추가(`FailedTransaction`,
  `TraceLog`, `FailedTxDetail`, `FailedTxTrendPoint`, `PaginationMeta`,
  `TotalPaginatedResponse<T>`, `TimeBucket`), contract.ts에 런타임 파서 + 새
  envelope 3개(`parseFailedTxDetailEnvelope`/`parseFailedTxListEnvelope`/
  `parseFailedTxTimeseriesEnvelope`), hooks.ts에 TanStack Query 3개
  (`useFailedTxDetail`/`useFailedTxList`/`useFailedTxTimeseries`). ErrorCategory의
  PascalCase↔SCREAMING_SNAKE 비대칭은 기존 `normalizeErrorCategory` 재사용으로
  흡수. 단위테스트 4건 추가(call_tree 트리·total·PascalCase 정규화·malformed throw)
  로 round-trip 가드.

- **T02 (FailedTx 페이지 3 섹션)** — 기존 카테고리 합계 시각화는 보존(Overview 무회귀).
  변경:
  - "Time-axis of recent failures"(`most_recent_failure` 기반 휴리스틱) →
    **Failure trend (real timeseries)** `AreaChart`로 교체. `useFailedTxTimeseries`
    가 (bucket × error_category) 카운트 반환 → wide 행으로 pivot해서 카테고리별
    stacked area. lookback에 따라 interval 자동(≤7d=hour / ≤90d=day / 그 외=week).
  - "Sample signals" + "Investigation drill-down" 두 카드 → **Browse failed
    transactions** (`useFailedTxList`, `total` 노출, prev/next 페이지네이션,
    LIMIT=20) + **Tx inspection** drill-down(`useFailedTxDetail`, 행 클릭 시 URL
    `?tx=`로 활성화, 평탄 call_tree를 `call_depth` 들여쓰기 + `call_tree_truncated`
    경고 배지). 기존 traders/pools 링크는 별도 작은 카드로 보존.
  - URL 필터에 `offset`/`tx` 추가. category/window 변경 시 `offset=0`로 자동 리셋.

**리뷰 결과**
- 기존 `App.smoke.test.tsx`의 "failed-tx 페이지 route 렌더" 케이스 무회귀 통과 → page
  의 기본 import/구조 안전성 자동 가드.
- 빌드 산출물: 899 modules transformed, dist 정상 (charts chunk 411KB는 Recharts
  표준 — 분리 chunk라 초기 로드 영향 제한).

**리뷰 매핑**
- ROADMAP **FE-WIRE (R2)** → REALIZED (M001 후속 폴리시 종결).

**의도적 제외 (FE-WIRE2로 분리)**
- 알림 구독 UI(`POST /v1/alert-subscriptions` + 시크릿 1회 노출·회전 흐름)
- Pools/Traders 신규 매핑(우선순위 낮음, 별도 슬라이스 가치)

**잔여 한계 (정직)**
- 라이브 데이터 표시는 docker postgres+api 대상 dev 서버 클릭으로만 검증 가능
  (CI에 머문 자동 검증은 typecheck + 단위 + 빌드까지).
- DataTable의 mono-truncation은 시각적이라 자동 가드 없음.

**Reassess**: ROADMAP "FE-WIRE (R2)" 항목 제거. 남은 백로그는 DNS-rebinding
SSRF / 임계율 집계 / 알림 UI(FE-WIRE2 후속) / M003 출하용 S09.
