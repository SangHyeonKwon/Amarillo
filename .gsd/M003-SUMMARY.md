---
milestone: M003
title: Actionable Alerts + On-chain × Off-chain Join Example
status: SHIPPED
date: 2026-05-21
slices: [S08, S09, HARDEN, HARDEN2]
gate: pass   # fmt clean · clippy --workspace -D warnings 0 · indexer 36/36 · db lib 14/14 · db ignored 13/13 · verify 3종 ALL PASS · web typecheck/test/build OK
decisions: [D011, D012, D013]
---

# M003 — Actionable Alerts + On-chain × Off-chain Join · SHIPPED

출하 정의(REQUIREMENTS#M003): **"실패 패턴 구독 → 웹훅으로 푸시. 온체인 × 비공개
데이터 조인 예시 1건."**

## 수용 기준 — 항목별 ✅ (증빙)

### S08 — 실패 패턴 구독 + 웹훅 전송 ✅
- `POST /v1/alert-subscriptions {webhook_url, error_category?, to_addr?}` →
  생성. 비-https / loopback / RFC1918 / link-local / 메타데이터 IP / IPv4-mapped
  IPv6 / `localhost`·`*.local` 거부는 모두 **400**. `GET` 목록(secret 미노출),
  `DELETE /{id}` soft 비활성화. **`POST /{id}/rotate-secret`**(HARDEN2-T02)으로
  시크릿 회전 — 같은 1회 노출 계약. 증빙: `verify-alerts.sh` ALL PASS + db
  통합 3개(매칭/claim/회전) + SSRF 14단위 + receiver 검증 docs(Node/Python).
- 디스패처가 매칭 실패 tx에 대해 **HMAC-SHA256(32B 디코드 키)** 서명된 POST를
  **정확히 1회**(claim outbox 패턴) 전송, 실패는 재시도·기록, 다중 dispatcher
  안전(HARDEN-T02). bounded 동시성(HARDEN-T03/M2)으로 워스트 ~17min → ~1.7min.
  증빙: indexer 36 단위(서명/claim/SSRF/bounded) + wire e2e mock receiver 자동
  + claim PG 통합 + clippy --workspace 0.
- 마이그레이션 멱등, 신규 public `///`, prod `unwrap()` 0, 시크릿(`signing_secret`,
  `WS_URL`, webhook URL) **로그 미출력**. last_error는 `redact_urls` + 500자 캡.

### S09 — 온체인 × 비공개 데이터 조인 예시 1건 ✅
- `contract_label` 테이블(address → label, optional owner_id) 멱등 시드(Uniswap V3
  router/factory + pool 자동 매핑). `GET /v1/analytics/failed-tx/by-label`이
  `failed_transaction ⨝ transaction ⨝ contract_label` 결과를 (라벨, 주소)별로
  `total_failures` + `by_category` 맵으로 노출. **Dune이 구조적으로 못 하는**
  consumer-specific 라벨 조인의 1 케이스. 증빙: `verify-failed-tx-by-label.sh`
  ALL PASS(모양 + pivot invariant + 400 + empty owner), db 통합 1(public/alice/
  nobody/future), web pivot 단위 + FailedTx 페이지의 "Failures by labeled
  contract" 카드.

### 공통 비기능 ✅
- prod `unwrap()` 0, 파라미터화 SQL 100%, 신규 마이그레이션 멱등(S08·S09 모두
  `IF NOT EXISTS`+ `BEGIN/COMMIT` + `ON CONFLICT`), 신규 public `///` 문서,
  통합 검증 + HTTP 검증 + Web 검증 모두 자동. 시크릿 안전(메모리·로그·캐시 어디
  에도 미저장 — 모달 닫기 시 `mutation.reset()`까지).

## 최종 게이트 (2026-05-21, 새로 재실행 — KNOWLEDGE S04 규칙)

- `cargo fmt --check` (workspace) — clean
- `cargo clippy --workspace -- -D warnings` — 0
- `cargo test -p indexer` — **36/36**
- `cargo test -p db --lib` — **14/14** (SSRF guard)
- `cargo test -p db -- --ignored` — **13/13** (alerts 3 + failed_tx 8 + labels 1 + rollback 1)
- `bash scripts/verify-failed-tx.sh` — ALL PASS
- `bash scripts/verify-alerts.sh` — ALL PASS
- `bash scripts/verify-failed-tx-by-label.sh` — ALL PASS
- `cd web && npm run typecheck && npm run test && npm run build` — **17/17 + 900 modules**

## 슬라이스

- **S08** 실패 패턴 구독 + 웹훅 전송 `[untapped]` — outbox 디스패처(D012). → S08-SUMMARY.md
- **S09** 컨트랙트 라벨 × 실패 조인 `[untapped]` — D013 결정. → S09-SUMMARY.md
- **HARDEN** 운영 하드닝 `[weak-spot]` — follow lag/cancel + outbox claim + bounded parallelism. → HARDEN-SUMMARY.md
- **HARDEN2** URL masking + secret rotation `[weak-spot]`. → HARDEN2-SUMMARY.md
- **FE-WIRE / FE-WIRE2** 대시보드 결선(실패 분석 + 알림 구독 UI). → FE-WIRE-SUMMARY.md / FE-WIRE2-SUMMARY.md

## 정직한 한계 / 잔여

- **라이브 webhook 전송 자동 검증 부재**: 실제 수신자 부재로 dispatcher → wire
  POST의 라이브 e2e는 mock receiver(인라인 TcpListener)로 *서명 계약*만 자동.
  실제 수신측 인프라 측 통합은 운영자 책임 — `docs/api-alerts.md`의 Node/Python
  검증 예시로 위임.
- **컨트랙트 라벨 시연 데이터**: docker 기본 시드와 라벨 매칭 행이 0건일 수
  있음(시드 데이터 의존). by-label 엔드포인트는 빈 결과를 정상으로 받음.
  실제 의미 검증은 자기 라벨/실패 데이터 도입 후 수동.
- **인증 미연결**: `alert_subscription`/`contract_label.owner_id` 컬럼은 멀티-
  테넌시 prep, 인증 도입은 별 단위(D008 일관 — 프레임워크 도입은 그 자체로 한
  유닛 이상).
- **잔여 백로그**: DNS-time IP 재바인딩 SSRF / 임계율 집계 / Pools/Traders 페이지
  신규 API 매핑 — 모두 단독 PR 단위.

## Reassess

ROADMAP M003 `[x] SHIPPED`. M004 분해는 *다음 지시 시*에만(GSD-2: 출하 전 분해
금지). 차별화 해자 분기:
- 라벨 종류 확장(봇 자기-봇 / 거래소 KYC) — S09의 일반화로 자연 가능
- 임계율 집계 알림(D012 MVP 제외분) — 봇 운영자 페르소나 직격
- DNS-rebinding SSRF — 보안 잔여 닫기
- FE-WIRE 후속(Pools/Traders 매핑) — 우선순위 낮음

M001·M002·M003 모두 출하 완료 = 제품의 첫 *완전한* 수직 표면(데이터→실시간→
알림→비공개 조인)이 코드로 박혀 있음. 다음 호흡은 무엇을 *깊게* 만들지의 결정.
