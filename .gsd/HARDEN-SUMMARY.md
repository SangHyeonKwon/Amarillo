---
slice: HARDEN
title: 운영 하드닝 (S07/S08 백로그 정정 단위)
status: done
edge: weak-spot
milestone: cross-cutting (M002 + M003/S08)
tasks: [T01, T02, T03]
gate: pass             # fmt clean · clippy --workspace -D warnings 0 · -p indexer 30/30 · -p db --lib 14/14 · -p db --ignored 11/11 · verify-alerts ALL PASS
migrations: none
decision: none new (D012 REALIZED 갱신만)
artifacts:
  - crates/indexer/src/worker.rs            # cap_range_to + AtomicBool cancel + watcher + 6 단위테스트
  - crates/indexer/src/main.rs              # 고정-범위 호출이 영구 false flag로 무회귀
  - crates/db/src/queries.rs                # find_pending_alert_matches 시그처 + try_claim_alert_match
  - crates/db/tests/alerts.rs               # alert_claim_is_atomic_and_handles_stale (5 시나리오)
  - crates/indexer/src/alerts.rs            # dispatch_item + JoinSet 동시성 + wire_signed_post e2e
  - docs/realtime-follow.md                 # R3/R4 REALIZED
  - docs/api-alerts.md                      # M1 multi-worker + M2 parallelism REALIZED
verification_constraint: "라이브 RPC/WS/수신자는 여전히 환경 의존. wire 서명 계약은 mock receiver(tokio TcpListener) e2e로 처음 자동화. R4·M1·M2의 런타임 race는 동시 워커 실증보다는 SQL 시맨틱·JoinSet 패턴 + 단위/통합 테스트가 1차 증빙."
---

# HARDEN — 무엇이 실제로 일어났나 (백로그 정정)

S07-PLAN R3/R4 + S08 리뷰 M1/M2/L3 — 정확성 결함은 아니지만 운영상 실재하는
하드닝 항목들을 한 슬라이스로 묶어 정리. 마이그레이션 0, D012는 deviation 갱신.

- **T01 (follow 안전성, R3+R4)**
  - 순수 `cap_range_to(from, to, cap_blocks)` + `FOLLOW_CYCLE_BLOCK_CAP = 500`
    — 큰 lag에서 한 사이클이 거대 청크여서 reorg 체크가 미뤄지던 문제 해소.
    잔여 범위는 다음 사이클에서 이어 진행(6 단위테스트: under/equal/over cap/
    single block/cap=0 no-op/u64::MAX saturating).
  - `index_range`가 `&AtomicBool` cancel 플래그를 받아 청크 사이에서 체크.
    follow가 `tokio::signal::ctrl_c`를 watcher 태스크로 받아 플래그를 켜고,
    `index_range`는 ≤ `batch_size` 블록 내에 graceful 종료. per-chunk
    checkpoint가 부분 진행을 보존. 고정-범위 모드는 영구 false 플래그로 무회귀.

- **T02 (dispatcher 안전성, M1)**
  - `try_claim_alert_match`로 락 없는 원자적 토큰 발급:
    `INSERT … ON CONFLICT DO UPDATE WHERE (status='failed' OR stale-claimed)`.
    `alert_delivery` PK가 race 보장, WHERE가 delivered·fresh-claimed 충돌을
    필터. HTTP POST는 락 밖.
  - `find_pending_alert_matches`도 같은 `CLAIM_STALE_AFTER_SECS = 60` 공유
    (anti-join이 fresh-claimed 제외) — 두 함수가 한 기준이라 divergence 불가.
  - **crash 자동 복구**: 잡고 죽은 워커의 'claimed' 행은 60s 후 다음 사이클
    에서 재claim. 5 시나리오 통합테스트(new/fresh conflict/stale=0 recovery/
    delivered permanent block/failed retry) 통과.

- **T03 (dispatcher 동시성 + e2e, M2+L3)**
  - `dispatch_item` 헬퍼(소유 인자, `'static`) + `tokio::task::JoinSet`로
    prime-N + drain-refill → 항상 ≤ `MAX_CONCURRENT_POSTS = 10` 동시. 워스트
    케이스 ~17min → ~1.7min(수신자 응답에 묶임).
  - 한 task의 panic/내부 에러는 `tracing::error!` + `metrics.failed++`로
    격리 — best-effort 알림이라 사이클 전체를 죽이지 않음.
  - `DispatchOutcome` 열거로 결과 집계, `claim_skipped`는 정상 분산 처리 신호.
  - **mock receiver e2e** (`wire_signed_post_roundtrips_to_receiver`,
    `#[ignore]` 없음 — 로컬 TCP만 필요): tokio `TcpListener` 인라인 fixture가
    1 connection 수락해 요청 파싱 → 본문에 HMAC 재계산 → 디스패처 서명과 일치
    하면 200. **wire 서명 계약**(`X-Amarillo-Signature: sha256=<hex>` /
    `Content-Type: application/json` / POST / 본문 round-trip) 자동 가드.

**리뷰 정정 매핑**
- S07-PLAN **R3** (per-iter range cap), **R4** (ctrl_c granularity) → T01 REALIZED
- S08 리뷰 **M1** (concurrent dispatcher race) → T02 REALIZED
- S08 리뷰 **M2** (sequential POST 17min) → T03 REALIZED
- S08 리뷰 **L3** (wire 서명 자동 가드 부재) → T03 REALIZED (mock receiver)

**정직한 한계 (잔여)**
- mock receiver e2e는 *wire 서명 계약*만 검증. SSRF 가드가 loopback을 막아서
  production `dispatch_once` 전체 흐름을 localhost로 돌리는 건 불가(가드 우회 =
  보안 회귀라 거부). SSRF 통합은 14 SSRF 단위 + `verify-alerts.sh`의 5×400
  rejection이 별도 가드.
- 라이브 follow + 실제 RPC reorg + 실제 webhook 수신자 e2e는 여전히 환경 의존
  (D009~D012 일관) — docs의 수동 스모크 절차로 위임.
- **다중 dispatcher 라이브 race**는 SQL 시맨틱(PK + ON CONFLICT WHERE) +
  통합테스트로 *정합성*은 박혀 있으나, 실제 두 프로세스가 동시에 돌 때의
  관찰은 운영 측 책임.
- DNS 시점 IP 재바인딩(SSRF 잔여) — 본 슬라이스 범위 밖, 별도 백로그(연결 시점
  IP 검사).
- L1 부분 처리(에러 메시지 500자 캡)는 됐지만 webhook URL 마스킹은 여전히
  백로그.

**Reassess**: ROADMAP 백로그에서 위 5개 정정 항목 제거. 남은 백로그:
DNS-time SSRF, URL masking, secret rotation, rate aggregation, S09(M003 ship gate),
FE-WIRE. M003 출하는 여전히 S09 분해를 기다림.
