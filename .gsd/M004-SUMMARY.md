---
milestone: M004
title: Diagnostic Depth (+ developer product surface)
status: SHIPPED
date: 2026-05-21
slices: [S10, S11, S12, S13]
gate: pass   # fmt clean · clippy --workspace -D warnings 0 · -p indexer 36/36 · -p db --lib 14/14 · -p db --ignored 22/22 · verify 3종 ALL PASS · web typecheck/test 26/build 900 modules · TS tsc --noEmit clean · Python py_compile clean
decisions: [D014, D015, D016, D017]
ship_definition: "임의 실패 tx에 대해 *어디서 / 어떤 함수가 / 왜 + 어떻게* 실패했는지를 단건 응답 한 번에 정확하게. dApp 개발자가 카피해서 즉시 쓰는 클라이언트 표면까지."
---

# M004 — SHIPPED

출하 정의(REQUIREMENTS#M004): "임의의 실패 tx에 대해 *어디서/어떤 함수가/왜* 실패
했는지를 단건 호출에 정확하게." S13에서 *어떻게 쓰나*까지 확장 — 개발자가 한
호출 응답 + 카피 가능한 예시 코드로 *프로덕트로 사용*.

## 응답 한 호출에 4축

| 슬라이스 | 질문 | 표면 |
|---------|------|------|
| **S10** | *어디서* revert가 났나? | `root_cause` (trace_id ASC LIMIT 1 of error frames) |
| **S11** | *어떤 함수가* 실패했나? | `failing_function_decoded` (selector → name + signature) |
| **S12** | *왜* + *어떻게* 고치나? | `diagnosis` (message + recommended_action) |
| **S13** | *어떻게 쓰나*? | `examples/typescript-client/` + `examples/python-client/` + `docs/cookbook.md` |

## 수용 기준 (REQUIREMENTS.md#M004) — 항목별 ✅

| 기준 | 상태 | 증빙 |
|------|------|------|
| `/v1/failed-tx/{tx_hash}` 응답에 `root_cause: TraceFrame \| null` | ✅ | S10 — verify `ROOT OK trace_id=N matches first error frame` |
| `failing_function`이 selector + decoded name/signature 동반 | ✅ | S11 — verify `DECODED OK (name :: signature)` or `null` self-consistent |
| `error_category`별 사람 가독 진단 메시지 + (가능 시) recommended_action | ✅ | S12 — verify `DIAG OK msg="…"` 시드된 카테고리 항상 non-null |
| 위 정보가 모두 `/v1/failed-tx/{tx_hash}` 단일 호출에 | ✅ | 핸들러 1 endpoint, 4축 가산만 (기존 `call_tree`/`call_tree_truncated` 불변, D004 일관) |
| 통합테스트 + verify 스크립트 + 프론트 단건 화면이 갱신된 계약 노출 | ✅ | db --ignored +9건(S10·S11·S12 합) · `verify-failed-tx.sh` 의미 단언 3종 · FailedTx 페이지 Root cause / Failing function / Diagnosis 3 블록 |
| 비기능: prod unwrap 0 / 파라미터화 SQL / `///` doc / 멱등 마이그레이션 | ✅ | 모든 신규 코드 준수, 마이그레이션 3건(20240105/06/07) `IF NOT EXISTS` + `ON CONFLICT DO NOTHING` |
| (S13 추가) 카피 가능한 예시 클라이언트 + cookbook | ✅ | TS + Python 외부 의존 0, `docs/cookbook.md` 3 시나리오 3중 예시 |

## 최종 게이트 (2026-05-21, 새로 재실행 — KNOWLEDGE S04 Rule)

- `cargo fmt --check` (workspace) — clean
- `cargo clippy --workspace -- -D warnings` — 0
- `cargo test -p indexer` — **36/36**
- `cargo test -p db --lib` — **14/14**
- `cargo test -p db -- --ignored` — **22/22** (alerts 3 + failed_tx 10 + function_signature 4 + category_diagnosis 3 + labels 1 + rollback 1)
- `bash scripts/verify-failed-tx.sh` — ALL PASS (live `ROOT OK trace_id=16` + `DECODED OK (null …)` + `DIAG OK msg="…"`)
- `bash scripts/verify-alerts.sh` — ALL PASS
- `bash scripts/verify-failed-tx-by-label.sh` — ALL PASS
- `cd web && npm run typecheck && npm run test && npm run build` — clean / **26/26** / 900 modules
- `tsc --noEmit -p examples/typescript-client/tsconfig.json` — clean (외부 의존 0)
- `python3 -m py_compile examples/python-client/{client,examples}.py` — clean (python3.13 + python3.6 모두)

## 슬라이스

- **S10** 콜트리 루트코즈 어트리뷰션 `[untapped]` — D014 (M004 방향). → S10-SUMMARY.md
- **S11** selector → 함수명/시그니처 디코딩 `[weak-spot]` — D015 (자기시드 정책). → S11-SUMMARY.md
- **S12** 카테고리 진단 + 추천 액션 `[weak-spot]` — D016 (enum 분리). → S12-SUMMARY.md
- **S13** TS+Python 예시 클라이언트 + cookbook `[weak-spot]` — D017 (예시=SDK). → S13-SUMMARY.md

## 핵심 교훈 (KNOWLEDGE 후보)

- **응답 4축 누적 = 누적적 가산 패턴**. 한 엔드포인트에 새 필드를 *명시적 null로*
  계속 가산 (D004/D014/D016 일관). 깨지지 않는 클라이언트 + 점점 똑똑해지는 응답.
- **명시 `null` 정책**(silent default 금지) — backend가 필드를 "잊어버린 것"과
  "indexer가 데이터 없음"을 클라가 구별 가능. 프론트·예시 파서가 키 누락을 throw.
- **자기소유 시드 정책**(D015/D016 일관) — 외부 4byte.directory / 메시지 카탈로그
  의존 0. 운영자가 `INSERT … ON CONFLICT DO UPDATE`로 큐레이트 가능. 공개
  데이터셋의 garbage(typo·충돌·노이즈)를 import할 의무 없음.
- **예시 코드 = SDK = 동일**(D017) — 외부 의존 0 정책의 직접 결과. `cp`로
  "설치", semver / 게시 토큰 / CI / 종속 그래프 관리 비용 0. 첫 사용자가 npm 게시를
  요청하기 전엔 그 무게가 *낭비*.
- **verify는 시드 invariant까지**: S12에서 *시드된 카테고리는 non-null* 단언을
  verify 스크립트에 박아 시드 회귀를 즉시 잡음 — shape ≠ semantics 패턴의 확장.

## 정직한 한계 / 잔여 (M005 후보)

- **S11.1 ABI args 디코딩** — `failing_function_decoded`는 name/signature까지.
  typed value 추출(address/uint/dynamic bytes/nested tuples)은 ABI 타입 시스템
  도입이라 별 슬라이스. 본 슬라이스는 *함수 식별*까지가 dApp 개발자에게 큰 가치
  임을 확인.
- **S12.1 ErrorCategory enum 세분화** — 6 카테고리는 *조잡* (모든 UNAUTHORIZED가
  동일 메시지). enum 확장은 `ALTER TYPE` + classifier 룰 + 프론트 type union이라
  별 슬라이스. 본 슬라이스는 *기본선* 메시지·액션을 박았고, 운영자가 UPDATE로
  큐레이트 가능한 패턴이 자리잡음.
- **S13.1 npm / PyPI 게시** — 예시는 카피 가능, 정식 패키지는 별 슬라이스.
  semver / 게시 토큰 / CI / 종속 그래프 관리는 첫 사용자가 명시 요청하면 도입.
- **인증 미연결** (D008 일관) — `alert-subscriptions` 등 쓰기 엔드포인트에도 인증
  없음. 운영 배포 시 별 단위. `contract_label.owner_id` / `alert_subscription`
  컬럼은 멀티-테넌시 prep만.
- **라이브 메인넷 자동 회귀 부재** — 본 마일스톤 모든 검증은 docker compose
  시드 데이터 기반. 메인넷 트래픽 자동 회귀는 환경 부재로 불가능. README/cookbook
  에 수동 스모크 절차 명시.

## 백로그 (M001-ROADMAP 잔여)

- **DNS-time SSRF 검사** (custom DNS resolver, 단독 PR)
- **임계·율 집계 알림** (D012 MVP 제외분, 봇 운영자 페르소나)
- **Pools/Traders 페이지 신규 API 매핑** (FE 후속)
- 모두 단독 단위 — M005 분기 시 우선순위 결정.

## Reassess

ROADMAP M004 `[x] SHIPPED`. M001~M004 모두 출하 완료 — 제품의 *완전한 수직
표면*이 코드로 박혀 있음 (데이터 → 실시간 → 알림 → 비공개 조인 → 진단 깊이 →
개발자 표면). 다음 호흡:

- **M005 분기**: 봇 운영자 페르소나(임계율 집계) / 보안 잔여(DNS-rebind SSRF) /
  깊이 추가(S11.1·S12.1) / 패키지화(S13.1) 중 사용자 결정.
- **다른 페르소나·체인 확장**: D003(스코프 동결)을 다시 검토할지의 결정 자체가
  새 호흡.

GSD-2 원칙: 다음 마일스톤 분해는 *다음 지시 시*만(M004 출하 전 분해 금지 원칙
일관 적용).
