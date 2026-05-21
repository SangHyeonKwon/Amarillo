---
slice: EXAMPLES-ADMIN
title: AmarilloClient admin 메서드 (S15 후속, BACKLOG #1 자연 추적)
status: done
edge: weak-spot
milestone: standalone (M005 완결성)
tasks: [T01]
gate: pass             # TS tsc --noEmit clean · Python py_compile clean · cargo + web 코드 무변경 → 회귀 자동 없음
decision: 없음 (기존 D017 정책 일관)
artifacts:
  - examples/typescript-client/client.ts  # ContractLabel interface + CreateLabelBody + createContractLabel / deleteContractLabel 메서드
  - examples/python-client/client.py      # ContractLabel @dataclass + create_contract_label / delete_contract_label 메서드
  - docs/cookbook.md                      # 봇 운영자 step 1: curl 외에 TS/Python client.method 시연 + cleanup 단락
  - .gsd/BACKLOG.md                       # M005 후속 항목 제거 + 우선순위 표 갱신 (인증 미들웨어가 #1로)
verification_constraint: "examples는 typecheck/syntax까지만 자동 — 라이브 호출은 docker compose + 수동 스모크(README/cookbook 명시). admin 메서드 단위테스트 미추가(cookbook 시연 + tsc + py_compile로 충분, examples는 SDK 아닌 카피 가능 예시 정신)."
---

# EXAMPLES-ADMIN — 무엇이 실제로 일어났나

BACKLOG #1 (S15 후속 — AmarilloClient admin 메서드) 출하. S15에서 박힌 `POST/
DELETE /v1/contract-labels` 표면을 examples client에 마감해 cookbook 봇 운영자
playbook의 step 1을 *curl 외에 TS/Python client method로도* 시연 가능.

- **T01 (TS + Python admin 메서드 + cookbook step 1 확장 + BACKLOG 정리)**:
  - `examples/typescript-client/client.ts`:
    - `interface ContractLabel { address, label, owner_id, created_at }`
    - `interface CreateLabelBody { address, label, owner_id? }`
    - `AmarilloClient.createContractLabel(body)` — UPSERT, 응답 row 반환
    - `AmarilloClient.deleteContractLabel(address)` — `encodeURIComponent`로
      path 안전, 404는 `AmarilloError(404)`로 throw
  - `examples/python-client/client.py`:
    - `@dataclass(frozen=True) ContractLabel { address, label, owner_id, created_at }`
      + `from_dict`
    - `AmarilloClient.create_contract_label(address, label, owner_id=None)`
    - `AmarilloClient.delete_contract_label(address)` — `urllib.parse.quote`로
      path 안전, 404는 `AmarilloError(404)` raise
  - `docs/cookbook.md` 4번째 시나리오 step 1 확장:
    - 기존 curl 그대로 + TS `client.createContractLabel({...})` + Python
      `client.create_contract_label(...)` 추가
    - cleanup 단락 추가 — `deleteContractLabel` / `delete_contract_label` 사용법
      + 404 idempotency 시그널 명시
  - `BACKLOG.md` 정리:
    - "AmarilloClient admin 메서드" 항목 제거(본 슬라이스 출하)
    - 우선순위 표 재계산: 이전 #1 (admin 메서드)이 빠지면서 *인증 미들웨어*가
      #1로 격상(M005-SUMMARY가 끌어올린 별 마일스톤 후보). S11.1 / S12.1 /
      S13.1 / OS resolver 캐시 race / Pools-Traders FE는 그대로 강등.

**해자(D017) 정신 일관**
- 외부 의존 0 정책(D017) 유지 — 새 메서드도 *기존 `_request` 헬퍼*만 사용
  (stdlib fetch / urllib만).
- README 변경 없음 — README가 이미 "covers every /v1/* endpoint"라 명시,
  새 메서드 자동 포함.

**정직한 한계**
- 단위테스트 미추가 — examples는 *SDK 아닌 카피 가능 예시*, tsc + py_compile
  로 충분(D017 정신). 라이브 호출 검증은 docker compose + 수동 스모크 또는
  cookbook 따라하기.
- admin 메서드 *컴파일/타입* 검증까지 — `createContractLabel`이 실제로 201을
  내는지의 *라이브* 검증은 verify-failed-tx-by-label.sh (S15-T02)가 이미
  HTTP 레벨에서 수행. examples는 *호출 방법 시연* 책임.

**Reassess**: BACKLOG #1 제거 + 우선순위 표 갱신(인증 미들웨어가 #1로 격상).
M001~M005 출하 상태 변경 없음 — 본 슬라이스는 M005 표면 마감 단독 PR. 다음
호흡은 사용자 지시 시 BACKLOG.md 우선순위 표 활용 — 인증 미들웨어(별 마일스톤)
가 가장 직접, 또는 S11.1 ABI args 디코딩(dApp 깊이) / S12.1 enum 세분화 등.
