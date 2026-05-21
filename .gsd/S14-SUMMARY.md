---
slice: S14
title: 임계율 집계 알림 (M005 첫 슬라이스)
status: done
edge: untapped
milestone: M005
tasks: [T01, T02, T03]
gate: pass             # fmt clean · clippy --workspace 0 · -p indexer 36/36 · -p db --lib 14/14 · -p db --ignored 25/25 (alert_rate 3 신규) · verify 3종 ALL PASS · web typecheck/test 29/build 900 modules
decision: D018
artifacts:
  - migrations/20240108000001_add_alert_subscription_rate.sql  # sub_type + 3 rate 컬럼 + CHECK + alert_rate_dispatch 테이블
  - crates/db/src/models.rs                                    # AlertSubscription/AlertSubscriptionCreated 확장 + RateAlertMatch + From<AlertSubscription>
  - crates/db/src/queries.rs                                   # insert_alert_subscription_rate + find_pending_rate_alert_matches + record_rate_alert_dispatch (+ per_event 필터 추가)
  - crates/db/tests/alert_rate.rs                              # 통합테스트 3 (match→debounce→재match / per_event 제외 / NULL 필드)
  - crates/indexer/src/alerts.rs                               # build_rate_payload + dispatch_rate_item + dispatch_rate_once + dispatch_loop 통합
  - crates/api/src/routes/alerts.rs                            # CreateBody sub_type+rate / 분기 핸들러 / 잘못된 조합 400
  - scripts/verify-alerts.sh                                   # rate 시나리오(valid 201 / 4 bad combos 400 / per_event+rate fields 400 / sub_type=bogus 400 / GET 표시)
  - docs/api-alerts.md                                         # rate_threshold 절 (body/응답 페이로드/디바운스 시맨틱/Self-imposed scope)
  - web/src/api/types.ts                                       # AlertSubType + AlertSubscription/Created 확장 + CreateBody
  - web/src/api/contract.ts                                    # readAlertSubType + parser 두 곳 갱신
  - web/src/api/contract.test.ts                               # 신규 케이스 3 (rate list / rate created / bogus sub_type throw) + 기존 4 케이스 sub_type 추가
  - web/src/pages/Alerts.tsx                                   # sub_type 라디오 + 조건부 rate 폼 + 컬럼 Mode 표시
  - .gsd/DECISIONS.md                                          # D018 (M005 방향 + 컬럼 가산 + 디바운스 시간 기반)
verification_constraint: "rate 발송 라이브 시뮬은 시드 분포에 따라 가변 — verify는 sub 생성/응답 검증까지 자동, 발송 자체는 dispatcher 수동 스모크. 정확히-1회 보장은 race 윈도우 짧지만 비강건 — 결정적 idempotency 필요시 수신측에서 sub_id+match_count 페어 dedupe (D018 문서화)."
---

# S14 — 무엇이 실제로 일어났나

REQUIREMENTS#M005 1차 가산 — `alert_subscription`에 `sub_type` + 3 rate 메타
컬럼을 가산해 *rate_threshold* 모드를 지원. 봇 운영자가 *건별 노이즈* 대신
*급증 패턴*만 알림 받게 — 디스패처가 sub_type 분기로 처리.

- **T01 (마이그레이션 + 모델 + 쿼리 + 통합테스트 + D018)**: 멱등 마이그레이션
  `20240108000001_add_alert_subscription_rate.sql`:
  - `sub_type TEXT NOT NULL DEFAULT 'per_event' CHECK (sub_type IN ...)`
  - `threshold_count` / `threshold_window_secs` / `debounce_secs` `INT` 가산
  - CHECK 제약 `alert_subscription_rate_fields_chk`: per_event면 3 컬럼 모두 NULL,
    rate_threshold면 모두 NOT NULL + 양수/비음수 검증
  - `alert_rate_dispatch(dispatch_id PK, subscription_id FK CASCADE, dispatched_at,
    match_count, status, last_error)` 테이블 + `idx_alert_rate_dispatch_sub_time`
  - 기존 행은 default `'per_event'`로 완전 호환 (silent default 금지 정신 일관,
    명시 default).

  `AlertSubscription` / `AlertSubscriptionCreated` 모델에 4 컬럼 가산 +
  `impl From<AlertSubscription>` 추가로 핸들러 변환 한 줄(row.into()). `RateAlertMatch`
  내부 매칭 모델 신규. 새 쿼리 3건:
  - `insert_alert_subscription_rate(...)` — rate sub 명시 INSERT
  - `find_pending_rate_alert_matches(limit)` — 활성 rate sub × 시간 윈도우 내
    매칭 카운트 + 디바운스 검증 (NOT EXISTS alert_rate_dispatch + last_dispatched_at
    > NOW - debounce_secs) → COUNT >= threshold인 행만
  - `record_rate_alert_dispatch(sub_id, count, delivered, err?)` — 발송 기록
    (디바운스 검증의 *마지막 발송 시각* 출처)

  기존 `find_pending_alert_matches`에 `AND s.sub_type = 'per_event'` 추가 —
  rate sub이 per-event matcher에 잡혀 *건별 발송*까지 trigger되는 회귀 차단.

  통합테스트 3건(`crates/db/tests/alert_rate.rs`):
  - `rate_match_then_debounce_then_match_again` — count=3 >= threshold=2 → 매칭,
    record_rate_alert_dispatch 후 다시 호출 시 디바운스로 제외(완전 race-free
    flow 시연)
  - `rate_matcher_ignores_per_event_subscriptions` — per_event sub은 rate matcher
    에서 빠짐 (sub_type 필터 확인)
  - `per_event_sub_has_null_rate_fields_by_default` — 기존 insert_alert_subscription
    호출은 default per_event + 3 rate 필드 모두 NULL (backwards compat)

  D018 기록: 봇 운영자 페르소나, 컬럼 가산 정책(별 테이블 회피), 디바운스 시간
  기반, race-safe but not strictly exactly-once.

- **T02 (디스패처 + API + verify + docs)**: indexer/alerts.rs 확장:
  - `RateAlertPayload { subscription_id, sub_type, match_count, threshold_count,
    threshold_window_secs }` + `build_rate_payload` (per-event payload와 *다른
    스키마* — `sub_type` 필드로 receiver가 분기)
  - `dispatch_rate_item` — SSRF 가드 + hex decode + sign + POST +
    `record_rate_alert_dispatch`. claim 패턴 불필요(SQL 디바운스가 race-safe)
  - `dispatch_rate_once` — bounded concurrency JoinSet (per-event과 같은 패턴)
  - `dispatch_loop`에 `dispatch_rate_once` 호출 추가, DispatchMetrics에 rate
    카운터 3 추가

  api/routes/alerts.rs: `CreateBody`에 `sub_type` + 3 rate fields. 핸들러는
  sub_type 분기 + 잘못된 조합 모두 **400**:
  - per_event + rate 필드 = 400 ("must not carry threshold_count/...")
  - rate_threshold + 필수 누락 = 400 ("requires threshold_count/...")
  - threshold_count <= 0, threshold_window_secs <= 0, debounce_secs < 0 = 400
  - sub_type='bogus' = 400

  `verify-alerts.sh` 확장: 4가지 잘못된 rate 조합 + per_event+rate fields +
  sub_type=bogus 모두 400 단언 + rate 201 응답에 sub_type/rate fields 확인 +
  GET list에 rate fields 보임 + secret 무누설. 실측 *ALL PASS* + 기존 11
  케이스 무회귀.

  `docs/api-alerts.md`에 `sub_type='rate_threshold'` subsection: body/응답 페이로드
  /디바운스 시맨틱 race 노트 + Self-imposed scope (S14.1 rate ratio/trend는 별).

- **T03 (프론트 + S14 출하)**: `types.ts`에 `AlertSubType` + 4 필드 가산.
  `contract.ts`에 `readOptionalInteger`/`readAlertSubType` 헬퍼 + 두 parser 갱신
  (per_event/rate_threshold 모두 검증). `contract.test.ts`에 신규 3 케이스
  (rate list / rate created / bogus sub_type throw) + 기존 alert 정상 케이스
  4곳에 sub_type 필드 추가.

  `Alerts.tsx`:
  - state: subType + 3 rate 입력 (모두 string)
  - validate(): rate 모드 시 3 필드 모두 정수 + 양수/비음수 검증
  - 폼: sub_type 라디오 + 조건부 rate fields(threshold_count / window_secs /
    debounce_secs 입력) — rate 모드 진입 시 강조 borderLeft 표시
  - 목록 컬럼 "Mode" 추가: rate는 `Rate ≥ N / Ws` 배지 + `debounce Xs` 줄,
    per_event는 muted "Per event"

  S14-SUMMARY + ROADMAP M005 S14 `[x]`. M005는 `🚧 IN PROGRESS` 유지(S15/S16 남음).

**해자(D002·D012·D018)의 *새 페르소나* 진입이 코드로 박힘**
- 봇 운영자가 자기 봇 sub 생성 시 `sub_type='rate_threshold'` 모드로 시간 윈도우
  + 임계 + 디바운스 설정 → 정상 운영 노이즈는 *완전 무시*, 급증만 알림.
- 같은 webhook URL로 dApp 개발자(per_event)와 봇 운영자(rate_threshold)가 *공존*
  가능 — receiver가 페이로드의 `sub_type` 필드로 분기. 한 인프라 다중 페르소나.
- 디바운스 race 시맨틱 명시 + 수신측 idempotency 가이드(D018) — "strictly
  exactly-once를 약속하지 말되 사용자가 그걸 보강할 수 있게" 일관 정직.

**정직한 한계**
- race 시맨틱: 두 워커가 같은 sub을 동시에 매칭 시 1-2회 짧은 overlap 가능(SQL
  디바운스 검증이 INSERT 사이 race를 막지 못함). 영구 중복은 불가. *strictly
  exactly-once* 필요한 수신측은 `subscription_id + match_count` 페어 dedupe 권장
  (D018 문서화).
- rate 자체는 *시간 윈도우 카운트* 만 — 비율(`failed_tx / total_tx`)이나 추세(이전
  윈도우 대비 증가율) 계산은 별 슬라이스(S14.1 sketch, D018).
- 디바운스는 *시간 기반*만 — 카운트 기반(예: "10건당 1회") 미지원.
- 라이브 rate 발송은 시드 데이터 분포에 따라 가변 — verify는 sub 생성·응답
  검증까지 자동, 발송 자체는 dispatcher 수동 스모크(README/cookbook 미명시; S16
  에서 봇 운영자 cookbook 시나리오에서 보강 예정).
- 인증 미연결(D008 일관). rate sub도 인증 없이 생성 가능.

**M005 진행**
- S14 ✅ 임계율 집계 알림 (본 슬라이스)
- S15 / S16 `[sketch]` 유지 — 다음 지시에서 분해. 후보:
  - S15 봇 라벨 (자기 봇 식별, S09 contract_label 확장 또는 별 테이블)
  - S16 봇 운영자 cookbook 시나리오 (docs/cookbook.md에 rate sub 흐름 + 봇 라벨 + HMAC 검증)

**Reassess**: ROADMAP M005 S14 `[x]`, M005는 `🚧 IN PROGRESS` 유지. KNOWLEDGE
추가 후보: "race-safe but not exactly-once 패턴 — DB 디바운스 + 수신측 dedupe
가이드 = 정직한 책임 분할" — D018 정신을 일반화. BACKLOG.md의 #1 (임계율) 항목
제거 + S15/S16 추적은 ROADMAP에 유지.
