---
slice: S15
title: 봇 라벨 admin API + 봇 운영자 cookbook (M005 마감 슬라이스)
status: done
edge: weak-spot
milestone: M005
tasks: [T01, T02, T03]
gate: pass             # fmt clean · clippy --workspace 0 · -p indexer 36/36 · -p db --lib 14/14 · -p db --ignored 27/27 (labels 3 = 신규 2 + 기존 1) · verify 3종 ALL PASS · web typecheck/test/build OK
decision: D019
artifacts:
  - crates/db/src/queries.rs                    # upsert_contract_label (INSERT … ON CONFLICT DO UPDATE RETURNING)
  - crates/api/src/routes/contract_labels.rs    # POST/DELETE 핸들러 (인증 X, D008/D019)
  - crates/api/src/routes/mod.rs                # 라우트 등록
  - crates/db/tests/labels.rs                   # 통합테스트 2 신규 (upsert overwrite / delete idempotency)
  - scripts/verify-failed-tx-by-label.sh        # POST/DELETE round-trip + 4종 400/404 단언 (기존 3 무회귀)
  - docs/api-failed-tx.md                       # admin POST/DELETE 절 + auth-or-the-lack-of-it 박스
  - docs/cookbook.md                            # 4번째 시나리오 "Bot operator playbook" (4 step curl+TS+Python)
  - .gsd/DECISIONS.md                           # D019 (S15 = M005 마감, 봇 라벨 별 테이블 X, 인증 X 데모)
verification_constraint: "admin API는 인증 미부착 — 데모/운영자-only 네트워크 스코프(D008/D019). 운영 배포 시 별도 인증 미들웨어 필수, 본 슬라이스에선 docs/cookbook에 명시만. examples/typescript-client / examples/python-client는 본 슬라이스에서 갱신 없음 — cookbook이 curl 위주로 시연, AmarilloClient에 createContractLabel 추가는 별 슬라이스 가치(BACKLOG 자연 추적)."
---

# S15 — 무엇이 실제로 일어났나

M005 마감 슬라이스. S14에서 박힌 rate-threshold 알림 메커니즘 위에 **봇 운영자
가 자기 봇 라벨을 동적으로 등록**할 수 있는 admin 표면을 열고, cookbook에 봇
운영자 *end-to-end 흐름*을 보존. S16(별 cookbook 슬라이스) 흡수.

- **T01 (admin API + 통합테스트 + D019)**: 신규 쿼리 `upsert_contract_label`
  (`INSERT … ON CONFLICT (address) DO UPDATE SET label, owner_id RETURNING …`)
  로 *create-or-update* 한 호출에. 기존 `insert_contract_label`(ON CONFLICT DO
  NOTHING)은 시드 멱등 용도로 그대로 유지. 신규 핸들러
  `crates/api/src/routes/contract_labels.rs`:
  - `POST /v1/contract-labels`: address `0x+40hex` 정규화, label 비-빈+100자 캡,
    owner_id 100자 캡, 잘못된 입력 모두 **400**. UPSERT라 같은 address 재호출은
    rewrite + 201.
  - `DELETE /v1/contract-labels/{address}`: 잘못된 주소 **400**, 미존재 **404**,
    성공 **204**. 두 번째 DELETE는 404 (멱등 — 운영자가 retry 시 no-op 신호).
  - 인증 미부착 명시(D008/D019 데모 스코프).
  라우트 `routes/mod.rs`에 등록. 통합테스트 2건:
  - `upsert_contract_label_creates_then_overwrites` — 같은 address 두 번 호출
    시 두 번째 응답이 새 label/owner를 반환(UPSERT 시맨틱 확인)
  - `delete_contract_label_is_idempotent` — 첫 DELETE affected=1, 두 번째
    affected=0 (핸들러 404 매핑의 DB 기반)
  D019 결정 기록(별 테이블 X / 인증 X / 시드 X / 프론트 폼 X 4가지 *not in scope*).

- **T02 (verify + docs admin 절 + cookbook 4번째 시나리오)**:
  - `verify-failed-tx-by-label.sh` 6 신규 케이스: POST create 201 + lowercased
    + round-trip, POST upsert overwrite 확인, POST 잘못된 주소 400, POST 빈 label
    400, DELETE existing 204, DELETE same again 404, DELETE bad address 400.
    cleanup도 trap에 합쳐 안전. 기존 3 케이스 무회귀. **ALL PASS** 실측.
  - `docs/api-failed-tx.md` by-label 절에 `### POST /v1/contract-labels` +
    `### DELETE /v1/contract-labels/{address}` + `### Auth (or the lack of it)`
    3 subsection 추가. UPSERT 명시, 인증 미부착 정직.
  - `docs/cookbook.md` 4번째 시나리오 "Bot operator playbook" — 4 step:
    (1) 라벨 등록 curl + UPSERT 설명
    (2) rate sub 생성 (S14 cookbook scenario 2 재사용 + bot to_addr 강조)
    (3) 수신 receiver outline(Express + Flask) — `sub_type` 분기 + 페이지 vs 티켓
    (4) `GET by-label?owner=alice` — 자기 라벨만 분리
    + race 시맨틱 박스(D018 정직 재진술)

- **T03 (S15-SUMMARY + M005-SUMMARY + ROADMAP M005 SHIPPED + BACKLOG 정리)**:
  본 SUMMARY + M005-SUMMARY.md 작성 + ROADMAP M005 헤더 `✅ SHIPPED → M005-SUMMARY.md` +
  S15 `[x]` + BACKLOG.md 라벨 일관성 마무리(S15 출하 항목 제거 + 우선순위 표 갱신)
  + 최종 게이트 재실행.

**M005 마감 = 봇 운영자 페르소나 완결 (D018 + D019)**
- S14가 *알림 메커니즘* (rate sub + 디바운스 + 분리된 페이로드)
- S15가 *식별 메커니즘* (자기 봇 라벨 등록 + by-label?owner 분리) + *흐름 문서화* (cookbook 4 step)
- 한 webhook URL이 dApp 개발자(per_event)와 봇 운영자(rate_threshold)를 *동시* 서비스 가능
- 같은 `contract_label` 테이블이 공개 라벨(Uniswap router 등, S09)과 봇 라벨(S15)을 *owner_id*로 분리 — 인프라 통합, 시맨틱 분리

**정직한 한계**
- 인증 미부착 (D008/D019) — 운영 배포 시 외부 노출 전 인증 미들웨어 필수. 본
  슬라이스는 docs/cookbook에 *명시*만, 실제 도입은 별 슬라이스 이상.
- examples/typescript-client / examples/python-client에 `createContractLabel`
  메서드 *미추가* — cookbook이 curl 위주로 시연. AmarilloClient에 추가는 별
  슬라이스 가치(BACKLOG에 자연 추적, 미선언).
- 봇 라벨 시드 추가 없음 — 운영자가 admin API로 동적 등록 또는 SQL 직접.
- 프론트 폼 미추가 — 봇 운영자는 CLI/스크립트 사용자, dApp 개발자용 by-label
  카드는 이미 FailedTx 페이지에 있음.

**Reassess**: M005 ✅ SHIPPED 선언. M001~M005 모두 출하 완료 — 제품 표면은
*두 페르소나(dApp 개발자 + 봇 운영자)*가 한 API 한 cookbook으로 모두 도달.
M005-SUMMARY.md 작성 + ROADMAP 헤더 SHIPPED + BACKLOG 우선순위 갱신.

다음 호흡(M006 분기 또는 단독 슬라이스 — DNS-rebind SSRF / S11.1 ABI args /
S12.1 enum / S13.1 패키지 / Pools/Traders FE 중 사용자 결정) — BACKLOG.md 우선
순위 표 활용.
