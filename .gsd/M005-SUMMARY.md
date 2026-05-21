---
milestone: M005
title: Bot Operator Persona
status: SHIPPED
date: 2026-05-21
slices: [S14, S15]
gate: pass   # fmt clean · clippy --workspace -D warnings 0 · -p indexer 36/36 · -p db --lib 14/14 · -p db --ignored 27/27 · verify 3종 ALL PASS · web typecheck/test 29/build 900 modules · TS tsc clean · Python py_compile clean
decisions: [D018, D019]
ship_definition: "봇 운영자가 자기 봇의 실패 *패턴*을 *임계율* 알림으로 받고, 자기 봇 라벨을 동적으로 등록·관리해 분리된 분석을 받는다."
---

# M005 — SHIPPED

출하 정의(REQUIREMENTS#M005): "봇 운영자가 자기 봇의 실패 *패턴*을 *임계율*
알림으로 받는다 — 건별 노이즈 없이." S15에서 *자기 봇 식별·관리* 표면까지 확장
— 새 페르소나가 한 cookbook 흐름으로 *프로덕트로 사용*.

## 응답·표면 — 봇 운영자 완결 흐름

| 단계 | 슬라이스 | 표면 |
|------|---------|------|
| **식별** — 자기 봇 라벨 등록·관리 | **S15** | `POST/DELETE /v1/contract-labels` (UPSERT/idempotent) |
| **구독** — 자기 봇의 실패에 rate 알림 | **S14** | `POST /v1/alert-subscriptions {sub_type:'rate_threshold', threshold_count, threshold_window_secs, debounce_secs, to_addr}` |
| **수신** — webhook 검증 + sub_type 분기 | S14 docs | `X-Amarillo-Signature: sha256=<hex>` HMAC-SHA256 (raw body, 32-byte key) |
| **분석** — 자기 라벨만 분리된 실패 분포 | S09 + S15 | `GET /v1/analytics/failed-tx/by-label?owner=<bot-owner-id>` |
| **playbook** | S15 cookbook | docs/cookbook.md 4번째 시나리오 (4 step curl+TS+Python) |

## 수용 기준 (REQUIREMENTS.md#M005) — 항목별 ✅

| 기준 | 상태 | 증빙 |
|------|------|------|
| `alert_subscription`에 `sub_type` + 3 rate 메타 컬럼 가산 + CHECK 일관성 | ✅ | S14-T01 마이그레이션 + 통합테스트 3 |
| 디스패처가 rate sub의 윈도우 count >= threshold + debounce 검사 후 1회 webhook | ✅ | dispatch_rate_once + alert_rate_dispatch + 통합테스트 시연 |
| 잘못된 rate 조합(per_event+rate / rate 필수 누락 / 음수 등) 모두 **400** | ✅ | verify-alerts.sh 4 신규 case ALL PASS |
| 봇 라벨 admin API (POST/DELETE) + 잘못된 입력 400/404 | ✅ | S15-T01 핸들러 + verify-failed-tx-by-label.sh 6 신규 case ALL PASS |
| 같은 라벨 인프라(`contract_label`)로 공개 라벨(S09) + 봇 라벨(S15) 분리 | ✅ | `owner_id` 컬럼으로 분리, by-label?owner 필터 (S09 시연 무회귀) |
| cookbook에 봇 운영자 4 step end-to-end 시나리오 | ✅ | docs/cookbook.md 4번째 시나리오 (curl + TS receiver + Python receiver) |
| 비기능: prod unwrap 0 / 파라미터화 SQL / `///` doc / 멱등 마이그레이션 | ✅ | 모든 신규 코드 준수, 마이그레이션(20240108) `IF NOT EXISTS` + CHECK round-trip 멱등 |

## 최종 게이트 (2026-05-21, 새로 재실행 — KNOWLEDGE S04 Rule)

- `cargo fmt --check` (workspace) — clean
- `cargo clippy --workspace -- -D warnings` — 0
- `cargo test -p indexer` — **36/36**
- `cargo test -p db --lib` — **14/14**
- `cargo test -p db -- --ignored` — **27/27** (alerts 3 + alert_rate 3 + category_diagnosis 3 + failed_tx 10 + function_signature 4 + labels 3 [신규 upsert·delete idempotency 2 포함] + rollback 1)
- `bash scripts/verify-failed-tx.sh` — ALL PASS
- `bash scripts/verify-alerts.sh` — ALL PASS (S14 rate 시나리오 6 신규)
- `bash scripts/verify-failed-tx-by-label.sh` — ALL PASS (S15 admin 시나리오 6 신규)
- `cd web && npm run typecheck && npm run test && npm run build` — clean / **29/29** / 900 modules
- `tsc --noEmit -p examples/typescript-client/tsconfig.json` — clean
- `python3 -m py_compile examples/python-client/{client,examples}.py` — clean

## 슬라이스

- **S14** rate_threshold 알림 `[untapped]` — D018 (디바운스 시간 기반, race-safe-but-not-strictly-once). → S14-SUMMARY.md
- **S15** 봇 라벨 admin API + cookbook 봇 시나리오 `[weak-spot]` — D019 (S16 흡수, 인증 X 데모 스코프). → S15-SUMMARY.md

## 핵심 교훈 (KNOWLEDGE 후보)

- **race-safe but not exactly-once 패턴**(S14/D018) — DB 디바운스 + 수신측
  dedupe 가이드 = 정직한 책임 분할. 분산 시스템 정직성 패턴.
- **컬럼 가산 + sub_type enum 패턴**(D018) — 새 모드 추가 시 *별 테이블* 대신
  *같은 테이블에 mode 컬럼 + CHECK 일관성*. backwards compat + 인프라 통합.
  D004/D013/D015/D016/D018 공통 정신.
- **UPSERT separate from idempotent INSERT** (D019) — 시드 멱등(`ON CONFLICT
  DO NOTHING`)과 admin UPSERT(`ON CONFLICT DO UPDATE RETURNING`)는 다른 의미.
  같은 테이블에 두 쿼리 함수 공존이 자연 — *호출처가 다르면 쿼리도 다르게*.
- **인증 미부착 명시 정책**(D008 → D013 → D019) — 데모 스코프에서 인증은
  *별 슬라이스 가치*. 명시적 무인증 + 운영 가이드는 *솔직성*이지 *결함*이 아님.

## 정직한 한계 / 잔여 (M006 후보 또는 단독)

- **인증 미부착** — S15 admin API는 인증 없음. 운영 배포 시 별도 인증 미들웨어
  필수. *별 슬라이스 가치* — S15.1 또는 마일스톤 단위(D008 spirit).
- **rate ratio / trend 미지원**(S14.1 sketch, D018) — 절대 임계만. *비율*(실패율
  vs 전체) / *추세*(이전 윈도우 대비 증가율)는 별 슬라이스.
- **AmarilloClient `createContractLabel` 미추가** — cookbook 4 step의 라벨
  등록 step은 curl로만 시연(TS/Python는 분석·post-processing만). examples
  client 메서드 추가는 BACKLOG 자연 추적, 별 슬라이스.
- **봇 라벨 시드 추가 없음** — 운영자가 admin API로 동적 등록 또는 SQL 직접.
- **race 시맨틱 1-2회 짧은 overlap** (S14/D018) — strictly exactly-once 필요시
  수신측 `sub_id + match_count` dedupe (cookbook 명시).
- **라이브 메인넷 자동 회귀 부재** — 본 마일스톤 모든 검증은 docker compose
  시드 데이터 기반. 메인넷 트래픽 자동 회귀는 환경 부재로 불가능.

## 백로그 (BACKLOG.md 참조)

- **DNS-time SSRF 검사** (custom DNS resolver, 단독 PR)
- **S11.1 ABI args 디코딩 + root_cause.input 디코드** (M004 진단 깊이)
- **S12.1 ErrorCategory enum 세분화 v2** (M004 정밀도)
- **S13.1 npm/PyPI 패키지 게시** (M004 운영성)
- **Pools/Traders 페이지 신규 API 매핑** (FE 후속, 우선순위 낮음)
- **AmarilloClient admin 메서드 추가** (S15 후속, BACKLOG 신규)

모두 단독 단위 — M006 분기 시 우선순위 결정. BACKLOG.md 우선순위 표 활용.

## Reassess

ROADMAP M005 `[x] SHIPPED`. M001~M005 모두 출하 완료 — 제품의 *두 페르소나
완결*:

- **dApp 개발자** (M001~M004): 실패 → 진단(어디서/뭐가/왜+어떻게) → 카피
  가능한 클라이언트
- **봇 운영자** (M005): 자기 봇 식별 → rate 알림 → 자기 라벨 분리 분석 →
  cookbook end-to-end

다음 호흡:
- **M006 분기** (M005처럼 새 페르소나/유스케이스 도입) — 후보: 거래소 KYC 매핑
  / 자동화된 incident response / RPC 성능 대시보드 등. D003(스코프 동결) 재검토
  자체가 새 호흡.
- **잔여 백로그 처리** (DNS-rebind SSRF / S11.1 / S12.1 / S13.1 / FE) — BACKLOG.md
  우선순위 표 활용.
- **운영성 강화** — 인증 도입(전체 admin/write API 보호) — *별 마일스톤 또는
  단독 큰 슬라이스* (M005.1 또는 M006의 핵심 슬라이스).

GSD-2 원칙: 다음 마일스톤 분해는 *다음 지시 시*만(M005 출하 전 분해 금지 원칙
일관 적용 — 본 SHIPPED 선언으로 분해 가능).
