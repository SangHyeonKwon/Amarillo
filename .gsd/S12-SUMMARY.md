---
slice: S12
title: 카테고리 진단 메시지 + 추천 액션 (M004 셋째 슬라이스)
status: done
edge: weak-spot
milestone: M004
tasks: [T01, T02, T03]
gate: pass             # fmt clean · clippy --workspace 0 · -p indexer 36/36 · -p db --lib 14/14 · -p db --ignored 22/22 (category_diagnosis 3 신규 + function_signature 4 + alerts 3 + failed_tx 10 + labels 1 + rollback 1) · verify 3종 ALL PASS (DIAG OK 실측) · web typecheck/test/build OK
decision: D016
artifacts:
  - migrations/20240107000001_add_category_diagnosis.sql   # 멱등 + 6 카테고리 시드
  - crates/db/src/models.rs                                # CategoryDiagnosis + Diagnosis + FailedTxDetail.diagnosis + ErrorCategory::as_wire()
  - crates/db/src/queries.rs                               # get_category_diagnosis (정확 매칭)
  - crates/db/tests/category_diagnosis.rs                  # 통합테스트 3 (6 cat 시드 + slippage 액션 + 미존재 None)
  - crates/api/src/routes/failed_tx.rs                     # 핸들러 가산 (as_wire → lookup → Diagnosis)
  - scripts/verify-failed-tx.sh                            # DIAG semantics 단언 (seeded category → non-null)
  - docs/api-failed-tx.md                                  # diagnosis 절 + 시드 정책 + S12.1 enum 세분화 sketch
  - web/src/api/{types,contract}.ts                        # Diagnosis + parser (missing-key 거부)
  - web/src/api/contract.test.ts                           # 신규 케이스 3 (object/missing/malformed) + 기존 3 수정
  - web/src/pages/FailedTx.tsx                             # "Diagnosis" 강조 블록 (Root cause / call_tree 사이)
  - .gsd/DECISIONS.md                                      # D016 (스코프: 메시지+액션까지, enum 세분화 분리)
verification_constraint: "M004는 *진행 중* — S12 출하 후에도 S11.1/S12.1/S13 남음. UNKNOWN 카테고리로도 시드된 진단 메시지가 자동 노출되어 의미 검증이 자동 성립(DIAG OK 실측 - S11과 차이)."
---

# S12 — 무엇이 실제로 일어났나

REQUIREMENTS#M004 3차 가산 — `/v1/failed-tx/{tx_hash}` 응답에
`diagnosis: { message, recommended_action?, source? } | null`을 추가. `error_category`
6개에 대한 사람이 읽는 진단 메시지 + 추천 액션을 자기소유 시드로 매핑.
dApp 개발자에게 "왜 + 어떻게"를 한 호출에 제공. S10 *어디서* + S11 *어떤 함수가*
+ S12 *왜+어떻게* 누적 완성.

- **T01 (스키마 + 시드 + 모델 + 쿼리 + 통합테스트 + D016)**: 멱등 마이그레이션
  `20240107000001_add_category_diagnosis.sql` — `category_diagnosis(error_category PK,
  message, recommended_action?, source?, created_at)` + 6 카테고리 시드(UNKNOWN /
  INSUFFICIENT_BALANCE / SLIPPAGE_EXCEEDED / DEADLINE_EXPIRED / UNAUTHORIZED /
  TRANSFER_FAILED) 각 message + recommended_action + source='builtin'. PK는 TEXT
  로 단순화(Postgres enum 컬럼 회피 — 마이그레이션 영향 최소화). `ON CONFLICT
  (error_category) DO NOTHING`으로 멱등.

  `CategoryDiagnosis`(FromRow) + `Diagnosis`(응답용 합성, error_category·created_at
  제외) 모델 + `impl From<CategoryDiagnosis>` 변환. 쿼리 `get_category_diagnosis`는
  정확 매칭. 통합테스트 3건:
  - `all_six_categories_seeded` (6 카테고리 모두 lookup + message 비-빈 + source=builtin)
  - `slippage_diagnosis_has_recommended_action` (시드 정합성 — recommended_action 비-빈)
  - `nonexistent_category_is_none` (silent default 금지)

  D016 결정 기록: enum 세분화는 별 슬라이스(S12.1 sketch), 자기시드 정책, TEXT PK 채택 이유.

- **T02 (API 응답 가산 + verify + docs)**: `FailedTxDetail`에 `diagnosis: Option<
  Diagnosis>` 가산(`skip_serializing_if` 금지). `ErrorCategory::as_wire()` 메서드
  신규(SCREAMING_SNAKE wire form 단일 출처) — `error_category_to_sql`(private)과
  동일한 매핑이지만 public 노출. 핸들러는 `failed.error_category.as_wire()` →
  `get_category_diagnosis` → `Diagnosis::from`. 미매칭은 명시 `null`.

  `scripts/verify-failed-tx.sh`에 node 의미 단언 추가:
  - `diagnosis` 필드 존재 (`hasOwnProperty`)
  - null 또는 object
  - object면 `message` 비-빈, `recommended_action`/`source` 는 string 또는 null
  - **시드된 카테고리(6개 wire form)는 반드시 non-null** — 시드 정합성 자동 검증

  실측 GOOD tx 응답에서 `DIAG OK (msg="The exact failure mode could not be clas…")` —
  UNKNOWN 카테고리도 시드되어 있어 의미적 시연이 자동 통과(S11과 달리 null/object
  양쪽이 아니라 *명시적 의미 검증*이 성립).

  `docs/api-failed-tx.md`에 `diagnosis` 절 + 6 카테고리 모두 시드됨 명시 + 운영자
  `UPDATE` 가이드 + S12.1 enum 세분화 sketch 한 단락.

- **T03 (web + S12 출하)**: `types.ts`에 `Diagnosis` + `FailedTxDetail.diagnosis`.
  `parseDiagnosis`(shape 단언) + `parseFailedTxDetail`이 `"diagnosis" in obj`로
  missing-key 거부(S10/S11/D014 일관). `contract.test.ts`에 신규 3 케이스
  (object 정상 / missing key throw / malformed message throw) + 기존 정상 3개에
  `diagnosis: null` 추가, S11 throw 케이스는 그대로(검증 순서상 그 단계에서 throw).
  FailedTx 페이지의 Tx inspection 카드의 Root cause 박스와 call_tree 리스트 사이에
  "Diagnosis" 강조 블록 추가 — `diagnosis` 객체면 message + `▶ recommended_action`
  + source 배지, null이면 짧은 안내. 기존 카드 무회귀.

**해자(D002·D014·D015·D016)의 *넷째* 깊이가 코드로 박힘**
- 진단 메시지/추천 액션은 *consumer-specific*: 누가 dApp을 만들고 있느냐에 따라
  필요한 액션이 다르다(예: 봇 운영자는 다른 메시지). 운영자가 `INSERT INTO
  category_diagnosis ... ON CONFLICT DO UPDATE`로 자기 메시지로 큐레이트 가능.
- 6 카테고리 모두 builtin 시드 → 즉시 작동. 그러면서 운영자 커스터마이즈가
  설계 안에 박혀 있음(S09 라벨 / S11 시그니처와 같은 패턴 — 비공개 보조 데이터가
  차별 가치).
- Dune이 못 함: 진단 텍스트는 *데이터*가 아니라 *제품 의견*. 공개 데이터셋에 없음.

**정직한 한계**
- 카테고리가 6개로 *조잡*하다는 정밀도 한계가 남음 — 모든 `UNAUTHORIZED` 케이스가
  동일 메시지. enum 세분화로 정밀도 ↑는 별 슬라이스(S12.1 sketch, D016).
- 메시지/액션은 *비전문* 영어 1줄 — 운영자가 자기 dApp 컨텍스트로 커스터마이즈
  하는 것을 가정. builtin 시드는 기본선일 뿐.
- 인증 미연결(D008 일관) — `category_diagnosis` 관리용 HTTP 표면 없음(시드는
  마이그레이션·운영 SQL 직접).
- 라이브 메인넷 자동 회귀 부재 — 자기 시드 6 카테고리로 의미 단언.

**M004 진행**
- S10 ✅ root_cause attribution (*어디서*)
- S11 ✅ selector decoding (*어떤 함수가*)
- S12 ✅ category diagnosis + action (*왜 + 어떻게*) ← 본 슬라이스
- S11.1 / S12.1 / S13 `[sketch]` 유지 — 다음 지시에서 분해. 후보:
  - S11.1 ABI args 디코딩 + root_cause.input 디코드 (typed value)
  - S12.1 ErrorCategory enum 세분화 v2 (마이그레이션 + classifier)
  - S13 개발자 SDK/문서 (TS/Python 미니멈 클라이언트)

**Reassess**: ROADMAP M004 S12 `[x]`, M004는 `🚧 IN PROGRESS` 유지. M004의
3축 핵심(어디서/뭐가/왜+어떻게)이 모두 응답에 박혀 있음 — 응답 하나만 받으면
dApp 개발자는 즉시 문제 진단 + 액션 가능. **M004 가 그 자체로 출하 가능한
형태에 도달**(필수 acceptance criteria 충족) — 운영 깊이/SDK는 nice-to-have.
다음 호흡은 *마무리*(S13 SDK)냐 *깊이 추가*(S11.1/S12.1)냐의 사용자 결정.

KNOWLEDGE 추가 없음 — D016이 결정 자체에 기록(자기시드 정책 + TEXT PK + enum
세분화 분리). 백로그(DNS-rebinding SSRF / 임계율 집계 / Pools·Traders 매핑)는
단독 단위 유지.
