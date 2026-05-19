---
slice: S07
title: 실시간 하드닝/관측성 (M002 마무리)
status: done
edge: weak-spot
milestone: M002
tasks: [T01, T02, T03]
gate: pass            # cargo test -p indexer 18/18, -p db --ignored 9/9, clippy --workspace 0, fmt clean
migrations: none
decision: D011
artifacts:
  - crates/indexer/src/worker.rs        # FollowMetrics, resolve_trigger_mode/TriggerMode/spawn_head_ticks, classify_fork/next_scan_depth/REORG_SCAN_CAP, lazy detect_fork
  - crates/indexer/src/config.rs        # subscribe 필드, ws_url dead_code 해소
  - crates/indexer/src/main.rs          # --subscribe → resolve_trigger_mode 결선
  - docs/realtime-follow.md             # 관측성·구독·동적확대 절, R1 정정→실현
  - .gsd/DECISIONS.md                   # D011 + D010 REALIZED
  - .gsd/S06-SUMMARY.md, .gsd/KNOWLEDGE.md  # R1/R5 정정·교훈
verification_constraint: "라이브 RPC/WS 미보장 → 순수 결정함수 단위(next_target 5 + classify_fork/next_scan_depth 7 + resolve_trigger_mode 6 = 18) + rollback 통합(9/9)로 충족, 라이브 follow/subscribe/reorg는 docs 수동 스모크"
---

# S07 — 무엇이 실제로 일어났나 (계획 대비)

S07-PLAN T01→T03 그대로. 마이그레이션 없음. **이 슬라이스가 M002 출하.**

- **T01 (관측성)**: follow에 경량 `FollowMetrics`(cycle/blocks/reorgs/last_reorg_depth,
  프로세스 메모리) + 사이클당 구조화 1줄 `"follow cycle summary"`(lag·처리량·reorg·
  마지막 폴 시각). reorg 사이클은 WARN 2줄(fork/depth/reorgs_total). 신규 의존성 0,
  동작/타이밍/IO 불변(관측 전용). `rollback_from_block` 반환값(=깊이)을 비로소 활용.
- **T02 (eth_subscribe, D011)**: 순수 `resolve_trigger_mode(subscribe,ws_url)` →
  `TriggerMode`(트림·공백⇒Polling). `spawn_head_ticks`가 WS provider 소유 백그라운드
  태스크로 newHeads→mpsc 틱. follow는 `trigger`로 틱 vs sleep 분기(ctrl_c 항상 레이스).
  **무회귀 폴백**: WS 연결/구독 실패·스트림 종료 시 채널 닫힘→자동 폴링. `ws_url`
  `#[allow(dead_code)]` 해소. **시크릿 안전**: WS_URL은 env 전용·로그 절대 미출력
  (모드 라벨만, 테스트가 강제). 신규 의존성 0(alloy/tokio `full`이 ws/pubsub/mpsc 보유).
- **T03 (동적 확대 + M002 Validate)**: `find_fork_point` → 순수 `classify_fork`
  (NoReorg/Fork/Inconclusive/NeedDeeper) + `next_scan_depth`(×4·클램프·종료성) +
  `REORG_SCAN_CAP=4096`. `detect_fork` 재작성: tip부터 **lazy 조회**(정상=1 RPC,
  R2 해소), 전부 불일치면 cap까지 ×4 확대해 **진짜 최소 공통조상**까지 롤백
  (R1 under-delete 갭 제거). 안전규칙(불확실→무롤백) 불변.

**리뷰 이월 처리 (S05+S06 홀리스틱 리뷰)**
- **R1** (정직성, High): D010/S06-SUMMARY/docs/S07-PLAN의 "보수적=과삭제=안전"
  → under-delete로 정정(서술), **그리고 T03에서 갭 자체를 제거(실현)**. 자가발견.
- **R2** (비용, Med-High): 전량 prefetch → T03 lazy 조회로 해소(정상 1 RPC).
- **R3/R4** (백로그): per-iteration 범위 cap / `index_range` 중 ctrl_c 응답성 —
  하드닝 정밀화(정확성 결함 아님), S07-PLAN 백로그 유지.
- **R5** (상속): 마이그레이션 `BEGIN/COMMIT` 중첩 — 기존 컨벤션·무해, KNOWLEDGE 기록.

**핵심 설계/교훈 (정직 기록)**
- R1은 *내가 쓴 문서*의 역방향 서술이었고 자가 리뷰가 잡아냈다. 파괴적 정정의
  안전성은 "보수적/안전" 라벨이 아니라 **실패 방향(over/under)·의존 가정·그
  가정이 깨지는 조건**으로 서술해야 한다(KNOWLEDGE Rule). 자기 산출물도 리뷰 대상.
- async IO를 순수 결정에서 분리하는 패턴 확장(S05/S06): T02는 트리거↔액션 분리
  (`resolve_trigger_mode` 순수), T03은 lazy 조회를 얇은 드라이버로 두고 판정을
  `classify_fork`/`next_scan_depth` 순수로 — 둘 다 RPC/WS 없이 단위테스트.

**검증 한계 (정직)**: 라이브 follow/subscribe/reorg는 `RPC_URL`/`WS_URL` 필요(이
환경 미보장). 순수 결정 18 단위 + rollback 통합 9/9 + 컴파일/clippy가 1차 증빙,
실제 구독·폴백·동적확대는 `docs/realtime-follow.md` 수동 스모크 절차. 잔여 정확성
경계: 4096블록 초과 reorg만 best-effort floor(≈PoS finality 64배 — 사실상 불가,
이제 **명시된** 경계이지 미명시 가정 아님).

**Reassess**: ROADMAP S07 `[x]`·M002 `[x] SHIPPED`. M002 = S05(팔로워)+S06(reorg)+
S07(하드닝/관측성) 완료. M003(`[sketch]`)은 **다음 지시 시** 분해(GSD-2: 출하 후).
