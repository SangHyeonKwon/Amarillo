---
slice: S06
title: Reorg 감지·정정
status: done
edge: untapped
milestone: M002
tasks: [T01, T02, T03, T04]
gate: pass            # cargo test -p indexer 11/11, -p db --ignored 9/9, clippy 0, fmt clean
migrations: 20240103000001_add_block_hashes.sql (idempotent)
decision: D010
artifacts:
  - migrations/20240103000001_add_block_hashes.sql
  - crates/db/src/models.rs              # Block + block_hash/parent_hash
  - crates/db/src/queries.rs             # recent_block_hashes, rollback_from_block
  - crates/db/tests/rollback.rs          # idempotent + scoped 통합테스트
  - crates/indexer/src/worker.rs         # find_fork_point + detect_fork + follow 결선
  - docs/realtime-follow.md              # reorg 절
verification_constraint: "라이브 reorg는 RPC 필요 → 순수 find_fork_point 단위(6) + rollback 통합(1)로 충족, 라이브는 수동 스모크"
---

# S06 — 무엇이 실제로 일어났나 (계획 대비)

S06-PLAN T01→T04 그대로. 첫 마이그레이션 동반.

- **T01**: `block_hash`/`parent_hash` 멱등 마이그레이션 + 모델/insert/worker. 시드행 NULL,
  `/v1/blocks` 무회귀, FromRow `SELECT *` 정합.
- **T02**: 순수 `find_fork_point`(불확실 RPC→None 안전규칙) + 단위테스트 6 + `recent_block_hashes`.
- **T03**: 멱등 `rollback_from_block`(단일 트랜잭션, FK 역순 삭제, checkpoint 되감기) + 통합테스트.
- **T04**: follow 결선 — 매 폴링 `detect_fork`(async prefetch→순수 함수) → fork면 rollback→재인덱싱.

**사고와 교훈 (정직 기록)**: T03에서 테스트 픽스처 `FORK=9.999M`이 시드 18M보다 낮아
`rollback_from_block`이 **공유 dev DB 시드 전체를 삭제**. 함수는 정상, 픽스처 상수가 버그.
`FORK=99M` 수정 + `docker compose run --rm seed` 복구 + 재검증(9/9, verify ALL PASS).
KNOWLEDGE Rule 등록: 범위/`>=` 파괴연산 픽스처는 실제 시드 상한보다 확실히 위.

**핵심 설계 (D010 / KNOWLEDGE)**
- 불확실(RPC 실패/블록 부재) → **무롤백**(파괴적 false positive 방지)
- async 사전조회 → 순수 sync `find_fork_point` 주입(테스트성 보존, S05 패턴 확장)
- 깊은 reorg는 보수적 floor 롤백(과삭제 후 재인덱싱 = 안전)

**검증 한계(정직)**: 라이브 follow+reorg는 `RPC_URL` 필요 → 이 환경 미보장.
구성요소(find_fork_point 단위 6, rollback 통합 1, next_target 단위 5)로 정확성 충족,
결선은 컴파일+clippy/fmt, 라이브는 `docs/realtime-follow.md` 수동 스모크 절차.

**Reassess**: ROADMAP S06 `[x]`, S07 `[sketch]` 해제·S07-PLAN.md 분해. M002 = S05·S06 완료,
S07(하드닝/관측성)만 남음.
