# S15 — 봇 라벨 admin API + 봇 운영자 cookbook (M005 마감 슬라이스) · PLAN

Slice 목표: REQUIREMENTS#M005 마감 — `contract_label`(S09)의 admin 표면을
열어 봇 운영자가 *자기 봇 라벨을 동적으로 등록* 가능하게. cookbook에 봇 운영자
시나리오 4번째를 추가해 S14 rate sub + 라벨 등록 + by-label 분석 흐름을
*한 cookbook page* 에 보존. S16(별 cookbook 슬라이스) 흡수.

엣지: `[edge: weak-spot]`. risk: low. deps: M001~M004 + S09 (contract_label) +
S14 (rate sub). **M005 마감 슬라이스** — 본 슬라이스 출하 시 M005 ✅ SHIPPED 선언.

핵심 결정: **D019** (착수 시 이미 기록) — 봇 라벨 별 테이블 신설 없음, 인증
미부착(D008 데모 스코프), 봇 라벨 시드 추가 없음(운영자가 admin API로 동적 등록),
프론트 폼 미추가(봇 운영자는 CLI/스크립트 사용자).

검증 제약(D009~D018 일관): T01 통합 PG + clippy/fmt, T02 verify HTTP +
cookbook syntax/typecheck(예시 클라이언트 sub_type 신규 매개 추가 시), T03
M005-SUMMARY + ROADMAP M005 ✅ SHIPPED.

태스크: T01 → T02 → T03.

---

## T01 — API admin endpoints + 통합테스트

**Must-haves**
- *Truths*
  - `POST /v1/contract-labels`:
    - Body: `{ address: string, label: string, owner_id?: string }`
    - 검증: address는 `0x` + 40 hex(소문자 정규화), label은 비-빈 + 길이 캡(예: 100자),
      owner_id는 선택(있으면 길이 캡)
    - 잘못된 주소 → **400**, 잘못된 label/owner_id → **400**
    - 성공 → **201** + `{ data: { address, label, owner_id, created_at } }`
    - 이미 존재(same address) → ON CONFLICT DO NOTHING → 멱등 — 응답 201 with
      *기존 행* 반환 또는 200 with `existed: true` 표시. 단순화: **201** + 기존
      행 (멱등 가시 신호 없음, 호출자가 GET으로 확인).
  - `DELETE /v1/contract-labels/{address}`:
    - 주소 검증(잘못 → 400), 소문자 정규화
    - 미존재 → **404**, 존재 → **204** (멱등 — 두 번째 호출은 404)
  - 새 라우트 등록: `crates/api/src/routes/mod.rs` 또는 별 파일 `contract_labels.rs`.
  - 통합테스트 (`crates/db/tests/labels.rs` 추가 또는 신규 파일): POST round-trip,
    DELETE 멱등, 잘못된 주소 거부.
    - DB 쿼리는 이미 S09-T01에 있음(`insert_contract_label`/`delete_contract_label`).
      통합테스트는 그 쿼리 호출 + 결과 검증.
  - prod `unwrap()` 0 / 파라미터화 SQL / 신규 public `///` doc.
- *Artifacts*: `crates/api/src/routes/{contract_labels.rs,mod.rs}`,
  `crates/db/tests/labels.rs`(확장)
- *Key Links*: S09 `insert_contract_label` / `delete_contract_label` (이미 존재),
  S08 알림 핸들러(POST 패턴), S02 입력 검증 패턴

## T02 — verify 확장 + docs/api-failed-tx admin 절 + cookbook 4번째 시나리오 + examples

**Must-haves**
- *Truths*
  - `scripts/verify-failed-tx-by-label.sh` 확장:
    - POST 라벨 → 201 + 응답 단언(address lowercased, label/owner_id round-trip)
    - 같은 address 두 번 POST → 멱등(둘 다 OK 또는 명시 conflict 표시)
    - 잘못된 주소 POST → 400
    - DELETE 등록한 라벨 → 204
    - DELETE 같은 라벨 다시 → 404
    - 기존 GET 시나리오 무회귀
  - `docs/api-failed-tx.md`의 by-label 절에 *admin* subsection 추가:
    - POST/DELETE 시그니처 + 인증 없음 명시(D008/D019)
    - 운영자가 정식 게시 시 인증 미들웨어 추가 권고
  - `docs/cookbook.md` 4번째 시나리오 "Bot operator playbook":
    - Step 1: `POST /v1/contract-labels` — 자기 봇 라벨 등록 (curl + TS + Python)
    - Step 2: `POST /v1/alert-subscriptions` rate sub 생성 (S14, sub_type=rate_threshold,
      to_addr=자기 봇 주소) + signing_secret 1회 reveal
    - Step 3: 디스패처가 봇 실패율 임계 초과 시 rate webhook → 수신측 HMAC 검증
    - Step 4: `GET /v1/analytics/failed-tx/by-label?owner=내-운영자ID` — 자기 봇
      실패 분포 확인 (curl + TS + Python)
    - 한 단락: D019 정신 — 인증 X 데모 스코프, 운영자 책임
  - (선택) `examples/typescript-client/client.ts` + `examples/python-client/client.py`에
    `createContractLabel`/`deleteContractLabel` 메서드 추가 + typecheck/py_compile
    재검증.
- *Artifacts*: `scripts/verify-failed-tx-by-label.sh`, `docs/api-failed-tx.md`,
  `docs/cookbook.md`, `examples/*/client.{ts,py}`(선택)
- *Key Links*: S13 cookbook 3중 예시 패턴, S08 docs/api-alerts.md POST 절 패턴

## T03 — M005-SUMMARY + ROADMAP M005 ✅ SHIPPED + BACKLOG 정리 + S15-SUMMARY

**Must-haves**
- *Truths*
  - `.gsd/S15-SUMMARY.md`: T01/T02 산출, 게이트 evidence, 정직한 한계(인증 X,
    데모 스코프), M005 마감 표명.
  - `.gsd/M005-SUMMARY.md` 신규 — M001~M004 SUMMARY 형식 따라:
    - 출하 정의, 수용 기준 항목별 ✅ 표
    - 슬라이스 (S14 / S15)
    - 최종 게이트 재실행 (KNOWLEDGE S04 Rule)
    - 핵심 교훈
    - 정직한 한계(rate race / 인증 X / 봇 라벨 시드 X)
    - Reassess: M001~M005 모두 출하 → 다음 호흡(M004 잔여 S11.1/S12.1, DNS-rebind
      SSRF, dApp 운영성 강화 등)은 BACKLOG.md 우선순위 + 사용자 결정
  - `.gsd/M001-ROADMAP.md`:
    - M005 섹션 헤더 → `✅ SHIPPED → M005-SUMMARY.md`
    - S15 → `[x] DONE`
  - `.gsd/BACKLOG.md`: 정합 정리 — 우선순위 표 갱신 (M005 후속 항목 빠짐, M004
    잔여 + DNS-rebind + Pools/Traders가 남음), 라벨 일관성 마무리.
  - 최종 게이트 재실행: clippy/fmt/cargo test indexer/db lib/db --ignored/verify
    3종/web typecheck/test/build/TS tsc/Python py_compile 모두 green.
- *Reassess*: M005 ✅ SHIPPED. 다음 호흡은 사용자 결정 — BACKLOG.md 우선순위
  표 활용. 후보:
    - DNS-time SSRF(보안 잔여)
    - S11.1 ABI args 디코딩(dApp 진단 깊이)
    - S12.1 enum 세분화
    - S13.1 npm/PyPI 패키지 게시
    - 또는 새 마일스톤 분기
- *Artifacts*: `.gsd/{S15-SUMMARY,M005-SUMMARY,M001-ROADMAP,BACKLOG}.md`

---

## Slice 수용 (Complete = M005 SHIPPED)
- [ ] T01–T03 must-haves, 기존 모든 표면 무회귀
- [ ] DB 통합(`-p db --ignored`) + indexer + db lib + clippy --workspace + fmt 모두 green
- [ ] `verify-failed-tx-by-label.sh` ALL PASS (POST/DELETE 신규 단언 포함),
      `verify-alerts.sh` + `verify-failed-tx.sh` 무회귀
- [ ] `web` typecheck + test + build 통과 (web 변경 없음 → 무회귀 자동)
- [ ] TS examples `tsc --noEmit` + Python `py_compile` (선택 변경 시)
- [ ] REQUIREMENTS#M005 S15 항목 ✅ + S15-SUMMARY + M005-SUMMARY + ROADMAP M005 `[x] SHIPPED`
- [ ] BACKLOG.md 라벨 일관성 마무리
