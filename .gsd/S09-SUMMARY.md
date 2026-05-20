---
slice: S09
title: 온체인 × 비공개 라벨 조인 (M003 출하 게이트)
status: done
edge: untapped
milestone: M003
tasks: [T01, T02, T03]
gate: pass             # fmt clean · clippy --workspace 0 · -p indexer 36/36 · -p db --lib 14/14 · -p db --ignored 13/13 · verify 3종 ALL PASS · web typecheck/test 17/build OK
migrations: 20240105000001_add_contract_label.sql (idempotent + ON CONFLICT seed)
decision: D013
artifacts:
  - migrations/20240105000001_add_contract_label.sql              # contract_label 멱등 + Uniswap/pool 시드
  - crates/db/src/models.rs                                       # ContractLabel + FailedTxByLabelPoint
  - crates/db/src/queries.rs                                      # insert/delete/list_contract_labels + failed_tx_by_label_aggregate
  - crates/db/tests/labels.rs                                     # owner 필터·time window·pivot 시나리오 4
  - crates/api/src/routes/{failed_tx,mod}.rs                      # GET /v1/analytics/failed-tx/by-label
  - scripts/verify-failed-tx-by-label.sh                          # 모양·invariant·400·empty owner
  - docs/api-failed-tx.md                                         # by-label 절 + 해자 framing
  - web/src/api/{types,contract,hooks}.ts                         # FailedTxByLabelPoint + parser + useFailedTxByLabel
  - web/src/api/contract.test.ts                                  # pivot invariant + malformed throw
  - web/src/pages/FailedTx.tsx                                    # "Failures by labeled contract" 카드
  - .gsd/DECISIONS.md                                             # D013 (유스케이스 결정)
verification_constraint: "라이브 transaction.to_addr 매칭 시연은 docker 시드에 따라 0건 가능 — 모양·invariant·empty path는 자동 가드, 실제 의미 검증은 자기 라벨/실패 데이터 도입 후 수동."
---

# S09 — 무엇이 실제로 일어났나

REQUIREMENTS#M003의 "온체인 × 비공개 데이터 조인 예시 1건"을 **컨트랙트 라벨** 한
사례로 구현. Dune이 구조적으로 못 하는 부분(소비자별 비공개 라벨)을 한 엔드포인트
+ 한 카드로 시연한다.

- **T01 (스키마 + 시드 + DB 쿼리 + D013)**: 멱등 마이그레이션으로 `contract_label
  (address, label, owner_id?, created_at)` + `idx_contract_label_owner` 부분 인덱스.
  시드는 Uniswap V3 SwapRouter/Factory 글로벌 2건 + 기존 `pool` 테이블 행에서
  자동 매핑 (`SELECT … FROM pool ON CONFLICT DO NOTHING`). `failed_tx_by_label_
  aggregate(pool, owner?, from?, to?, limit)`가 `failed_transaction ⨝ transaction
  ⨝ contract_label` → (라벨, 주소, 카테고리) 그룹 카운트를 SQL로 받아 Rust에서
  (라벨, 주소)별로 카테고리 맵으로 **피벗**(sqlx `json` 피처 무도입). 통합테스트
  4 시나리오(public + alice + nobody + future window).
  D013 결정 기록: 봇/KYC 미선택 사유 + 검증 제약 명시.

- **T02 (API + verify + docs)**: `GET /v1/analytics/failed-tx/by-label?from=&to=&
  owner=&limit=` — `ApiResponse<FailedTxByLabelPoint[]>`. 잘못된 RFC3339는 400,
  unknown owner는 200 + 빈 배열, 빈 결과는 200 + 빈 배열. limit은 1..=200(기본 50).
  verify 스크립트가 모양/카테고리 합 = total_failures 불변/lowercased 0x+40hex
  자동 단언. docs는 *왜 Dune이 못 하나* framing 단락 추가.

- **T03 (web + M003 Milestone Validate)**: `FailedTxByLabelPoint` 타입 + 파서
  (`parseFailedTxByLabelEnvelope`, pivot 불변 단위테스트) + `useFailedTxByLabel`
  hook. FailedTx 페이지에 "Failures by labeled contract" 카드 — DataTable로
  라벨/주소(mid-trunc mono)/총 실패수/상위 4 카테고리 mini-badges. 빈 결과는
  친절 메시지(라벨은 있어도 매칭 실패 0건이면).
  **M003 Milestone Validate** 전 게이트 새로 1회 통과 — KNOWLEDGE S04 Rule
  대로 환경 드리프트 가드:
  - fmt --check clean · clippy --workspace -D warnings 0
  - `cargo test -p indexer` 36/36 · `-p db --lib` 14/14 · `-p db --ignored` 13/13
    (alerts 3 + failed_tx 8 + labels **1 new** + rollback 1)
  - `verify-failed-tx.sh` + `verify-alerts.sh` + `verify-failed-tx-by-label.sh`
    ALL PASS(포트 3001 충돌 시 `API_PORT=3099` 로 우회 — 절차 documented)
  - `npm run typecheck` clean · `npm run test` 17/17(15 + by-label pivot + malformed
    throw) · `npm run build` OK(900 modules)

**해자(D002·D013)의 *한 사례*가 코드로 박힘**
- Dune은 모든 데이터가 공개 영역이라야 작동. `contract_label`은 정의상 *비공개*
  (각 consumer가 다른 행을 가짐 — 봇 운영자/거래소/dApp 개발자 모두 동일 인프라
  + 다른 라벨로 응용 가능).
- 본 슬라이스는 컨트랙트 라벨 1종으로 박았고, 봇/KYC 라벨은 동일 스키마/엔드포인트
  패턴 재사용으로 후속 확장 가능 — 그 일반화 가치는 D013에 명시.

**정직한 한계**
- docker 기본 시드의 `transaction.to_addr`가 시드된 라벨 주소(Uniswap router/
  factory + pool addrs)와 실제로 매칭되는지는 시드 데이터에 달림 — verify
  스크립트가 `0 rows`도 정상으로 받아들이고 *모양·invariant*만 자동 단언.
  실제 의미 검증은 자기 라벨/실패 데이터 도입 후 수동 스모크.
- 인증 미연결(D008 일관). `owner_id` 컬럼은 멀티-테넌시 prep만 — HTTP 표면에
  인증 미부착. 운영 시 별 단위.
- `contract_label` 관리용 HTTP 표면 미구현(insert/delete는 db 함수만). 운영
  시드는 SQL 직접 또는 admin 도구. 의도적 (D008 spirit, 데모 스코프).

**Reassess**: ROADMAP M003 `[x] SHIPPED`. M004 분해는 *다음 지시 시*만(GSD-2:
출하 전 분해 금지). 남은 백로그(DNS-rebinding SSRF / 임계율 집계)는 단독 단위
유지. Pools/Traders 페이지 신규 API 매핑은 우선순위 낮음.
