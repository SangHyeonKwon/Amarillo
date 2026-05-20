# S09 — 온체인 × 비공개 라벨 조인 예시 (M003 출하 게이트) · PLAN

Slice 목표: REQUIREMENTS#M003의 "온체인 × 비공개 데이터 조인 예시 1건"을
**컨트랙트 라벨**(off-chain, 우리가 보관하는 1급 비공개 데이터)로 시연한다.
실패 인텔리전스(`failed_transaction`/`transaction`) × `contract_label`(신규)을
JOIN해 "라벨된 컨트랙트별 실패 분포"를 보여주는 한 화면을 만든다.

엣지: `[edge: untapped]`. risk: med. deps: M001 (이미 머지됨).
**M003 출하 게이트** — 본 슬라이스 마지막 task가 M003 milestone-validate.

핵심 결정: **D013** (착수 시 기록) — 유스케이스를 *컨트랙트 라벨*로 확정.
봇 운영자(자기 봇)/거래소(KYC)와의 trade-off는 D013에 기록.

검증 제약(D009~D012 일관): T01·T02는 통합 PG·verify HTTP + clippy/fmt.
T03은 typecheck+vitest+build, 시각 회귀는 docker postgres+api 대상 수동 스모크.
M003 milestone-validate는 KNOWLEDGE S04 Rule대로 전 게이트 새로 1회.

태스크: T01 → T02 → T03.

---

## T01 — 스키마 + 시드 + DB 쿼리 + 통합테스트 + D013

**Must-haves**
- *Truths*
  - 멱등 마이그레이션: `contract_label(address TEXT PK, label TEXT NOT NULL,
    owner_id TEXT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT NOW())` +
    `idx_contract_label_owner` partial index. `IF NOT EXISTS` + BEGIN/COMMIT
    + COMMENT ON.
  - 시드(SQL `sql/dml/`에 추가하거나 마이그레이션 안에 `INSERT … ON CONFLICT
    DO NOTHING` 동봉 — 데모 데이터라 마이그레이션 동봉이 응집력 높음): Uniswap V3
    SwapRouter (`0xE592…1564`) + Factory (`0x1F98…F984`) + 기존 `pool` 테이블의
    풀 주소들을 `pair_name` 라벨로 자동 매핑(`INSERT … FROM pool ON CONFLICT`).
  - 신규 모델 `ContractLabel` (FromRow + Serialize) + 응답용 합성
    `FailedTxByLabelPoint { label, address, total_failures, by_category }`.
  - DB 쿼리 `failed_tx_by_label_aggregate(pool, owner?: &str, from?, to?,
    limit) -> Vec<FailedTxByLabelPoint>` — `failed_transaction` ⨝ `transaction`
    ⨝ `contract_label` (label.address = transaction.to_addr, lowercased) →
    GROUP BY (label.label, label.address)에 카테고리별 JSON 집계
    (`jsonb_object_agg(error_category, count)` 또는 멀티 컬럼).
    파라미터화 SQL 엄수, `($1::TEXT IS NULL OR …)` 옵션필터 패턴.
  - 통합테스트(`crates/db/tests/labels.rs`): 라벨 1개 + 매칭 실패 1건 시드 →
    aggregate가 정확히 잡음 + 라벨 없는 to_addr는 제외 + owner 필터 동작 +
    teardown(CASCADE 또는 명시 DELETE).
  - **D013 기록**(`착수 시 DECISIONS`): "S09 유스케이스 = 컨트랙트 라벨" +
    봇/KYC 미선택 사유 + 검증 제약.
- *Artifacts*: `migrations/2024…_add_contract_label.sql`, `crates/db/src/models.rs`,
  `crates/db/src/queries.rs`, `crates/db/tests/labels.rs`, `.gsd/DECISIONS.md`
- *Key Links*: S06 멱등 마이그레이션 패턴, S02 옵션필터 SQL idiom (`$1::TEXT
  IS NULL OR …`), STH 통합테스트 하니스

## T02 — API by-label 엔드포인트 + verify + docs

**Must-haves**
- *Truths*
  - 라우트 `GET /v1/analytics/failed-tx/by-label?from=&to=&owner=&limit=`
    (limit 1..=200, 기본 50). 잘못된 RFC3339는 400. ApiResponse<배열> 봉투
    (목록 아니라 합산 결과라 `TotalPaginatedResponse` 안 씀 — D005 변형 회피).
  - `scripts/verify-failed-tx-by-label.sh`: 시드 시점 라벨 1건과 매칭되는 실패
    1건을 가정하고 happy 200·시간 필터 400을 검증. node로 본문 단언.
  - `docs/api-failed-tx.md`에 "By labeled contract" 절 추가(엔드포인트·예시·해자
    문구) + 기존 absolute curl 예시 추가.
- *Artifacts*: `crates/api/src/routes/{analytics,mod}.rs`(분리 가능 vs analytics
  내 추가; 후자가 응집력↑), `scripts/verify-failed-tx-by-label.sh`,
  `docs/api-failed-tx.md`
- *Key Links*: 기존 `analytics::failed_tx_analysis` / `failed_tx::failed_tx_timeseries`
  핸들러 스타일, S02 `parse_ts` 헬퍼

## T03 — 프론트 시각화 + M003 Milestone Validate

**Must-haves**
- *Truths*
  - `web/`: `types.ts` `FailedTxByLabelPoint` + `contract.ts` 파서 + envelope +
    `hooks.ts` `useFailedTxByLabel({from, to, owner, limit})`. contract.test 1
    케이스.
  - FailedTx 페이지에 새 카드 "Failures by labeled contract" — 시작 단순(테이블
    또는 가로 막대), 라벨된 컨트랙트가 0건이면 친절 빈 메시지("라벨 시드 데이터
    필요"+docs 링크). 무회귀.
  - **M003 Milestone Validate** (KNOWLEDGE S04 Rule — 전 게이트 새로 1회):
    `cargo fmt --check` · `cargo clippy --workspace -- -D warnings` ·
    `cargo test -p indexer` · `cargo test -p db --lib` · `cargo test -p db --
    --ignored` · `verify-failed-tx.sh` + `verify-alerts.sh` + 신규
    `verify-failed-tx-by-label.sh` · `cd web && npm run typecheck/test/build`.
  - `.gsd/S09-SUMMARY.md` + `.gsd/M003-SUMMARY.md` + `ROADMAP M003 [x] SHIPPED`.
- *Artifacts*: `web/src/api/{types,contract,hooks}.ts` + `contract.test.ts`,
  `web/src/pages/FailedTx.tsx`(섹션 추가), `.gsd/{S09-SUMMARY,M003-SUMMARY}.md`,
  `.gsd/M001-ROADMAP.md`
- *Reassess*: M003 출하 후 — M004 분해 *금지*(GSD-2: 다음 milestone은 명시
  지시에서만), 남은 백로그(DNS-rebinding SSRF / 임계율 집계 / 후속 라벨 다양화)
  현 위치 정리

---

## Slice 수용 (Complete = M003 SHIPPED)
- [ ] T01–T03 must-haves, 기존 `/v1/*`·페이지 무회귀
- [ ] DB 통합 + indexer + db lib + clippy + fmt 모두 green
- [ ] verify 스크립트 3개 ALL PASS (failed-tx, alerts, by-label)
- [ ] `web` typecheck + test(by-label 신규 contract 케이스 포함) + build 통과
- [ ] REQUIREMENTS#M003 S08 ∧ S09 항목별 ✅ + S09/M003 SUMMARY + ROADMAP M003 `[x]`
- [ ] 라이브 webhook 전송/수신은 여전히 환경 의존 → 수동 스모크 절차 유지
