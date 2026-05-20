# FE-WIRE — `web/` 대시보드 ↔ 신규 실패-인텔리전스 API · PLAN

Slice 목표: 이미 만들어진 `web/` (Vite+React+TS+TanStack Query+Recharts) 대시보드가
**M001 신규 엔드포인트(`/v1/failed-tx*`)를 실제로 소비**하도록 한다 — 백엔드 작업이
*클릭 가능한 표면*으로 변환되어야 개발자 페르소나가 사용 가능해진다. `web/`의
`contract.ts`가 S01 이전 작성이라 신규 응답 형태를 모름 → 계약 정합성부터.

엣지: `[edge: weak-spot]`. risk: low. deps: M001 (이미 머지됨). 새 결정 0,
마이그레이션 0, 신규 백엔드 변경 0.

PROJECT 스코프 단속: 프론트는 *후순위·최소한*만. 알림 구독 UI(시크릿 관리·회전
UX) + Pools/Traders 신규 매핑은 **본 슬라이스 밖** — 별도 슬라이스.

검증 제약: `npm run typecheck` + `npm run test`(vitest) + `npm run build`는 자동.
시각 회귀는 docker postgres+api 대상 로컬 dev 서버 클릭으로 수동(M001 verify
스크립트는 그대로 API 계약 백엔드 측 가드).

태스크: T01 → T02.

---

## T01 — contract sync + 신규 hooks

**Must-haves**
- *Truths*
  - `types.ts`: `FailedTransaction`(테이블 행), `TraceLog`, `FailedTxDetail`
    `{failed, call_tree, call_tree_truncated}`, `FailedTxTrendPoint`,
    `PaginationMeta`(`limit/offset/count/total`), `TotalPaginatedResponse<T>`,
    `TimeBucket`(`hour|day|week`) 추가. **기존 형(`FailedTxAnalysis` 등) 보존**
    (Overview 페이지 의존).
  - `contract.ts`: 위 형들의 런타임 파서 + envelope 파서. ErrorCategory는 기존
    `normalizeErrorCategory`가 PascalCase↔SCREAMING_SNAKE 비대칭 흡수(D002 패턴
    재사용). `BigDecimal`(`value` 등)은 `readDecimal` 재사용.
  - `hooks.ts`: `useFailedTxDetail(tx_hash)` / `useFailedTxList({category,from,to,
    limit,offset})` / `useFailedTxTimeseries({interval,from,to})`. 기존 hook 무회귀.
  - `contract.test.ts`: 신규 파서 3종 round-trip 단위(known fixture → 정상 + 잘못된
    shape → throw + ErrorCategory 두 와이어 모두 정규화 검증).
- *Artifacts*: `web/src/api/{types,contract,hooks}.ts` + `web/src/api/contract.test.ts`
- *Key Links*: 백엔드 `crates/api/src/response.rs`(envelopes), `crates/db/src/models.rs`
  (FailedTxDetail/TraceLog/FailedTxTrendPoint/ErrorCategory), `docs/api-failed-tx.md`

## T02 — Failed Tx 페이지 재결선 (list + drill-down + timeseries)

**Must-haves**
- *Truths*
  - `pages/FailedTx.tsx`를 3 섹션으로 — **Trend chart**(Recharts, `useFailedTxTimeseries`,
    기본 `interval=day` + 최근 7일 범위) / **필터 + 목록**(카테고리 select +
    from/to date input + 페이지네이션, `useFailedTxList`, `total` 노출) / **단건
    drill-down**(행 클릭 → detail panel, `useFailedTxDetail`로 revert 사유·
    `error_category`·평탄 call_tree 들여쓰기 표시, `call_tree_truncated`면 경고
    배지).
  - 기존 카테고리 합계 패널(`useFailedTxAnalysis`)은 "Category summary"로 보존
    (Overview는 무회귀).
  - 날짜 입력 `YYYY-MM-DD` → 백엔드 RFC3339(`T00:00:00Z`) 변환 헬퍼.
  - `npm run typecheck` / `npm run test` / `npm run build` green. 로컬 dev 서버
    수동 스모크 절차는 README 또는 `docs/` 1줄 추가(`docker compose up -d` +
    `cd web && npm run dev` → http://localhost:5173).
- *Artifacts*: `web/src/pages/FailedTx.tsx`(재구성), 필요 시 작은 components
  분리 (예: `FailedTxDetailPanel`, `TrendChart`), `README.md`/`docs/` 짧은 메모
- *Note*: **알림 구독 UI는 의도적 제외** — 시크릿 1회 노출·회전·복사 UX가 별 단위
  설계 필요. FE-WIRE2로 분리.

---

## Slice 수용 (Complete = FE-WIRE PR 머지)
- [ ] T01–T02 must-haves, 기존 페이지(Overview/Pools/Traders) 무회귀
- [ ] `cd web && npm run typecheck` clean · `npm run test` green(contract 신규 파서
      포함) · `npm run build` 성공
- [ ] FailedTx 페이지가 docker postgres+api 대상 dev 서버에서 실제 데이터로
      list → click → detail → trend 모두 렌더 (수동 스모크 1회)
- [ ] 알림 구독 UI 미포함 명시(FE-WIRE2 후속), README/CLAUDE.md 메모 1줄
