# S06 — Reorg 감지·정정 · PLAN

Slice 목표: `--follow` 중 체인 재구성(reorg)을 감지하고 영향 블록 데이터를
**멱등하게 정정**(롤백 + 재인덱싱)한다. "실무 인덱서 최난제" — M002 핵심 학습.

엣지: `[edge: untapped]`. risk: **high**. deps: S05.
⚠️ 프로젝트 **첫 마이그레이션** 필요 (현재 `block`에 해시 컬럼 없음 → reorg 감지 불가).

전제: S05 follow 동작. 검증 제약(D009 동일): 라이브 reorg는 RPC 의존 →
순수 fork 로직은 단위테스트, 정정은 PG 통합테스트(시뮬레이션), 라이브는 수동.

태스크: T01 → T02 → T03 → T04.

---

## T01 — 마이그레이션 + 모델: 블록 해시 저장

**Must-haves**
- *Truths*
  - `block`에 `block_hash`, `parent_hash` 컬럼(기존 행 위해 NULL 허용). 마이그레이션
    **멱등**(`ADD COLUMN IF NOT EXISTS`, `BEGIN`/`COMMIT`, CLAUDE.md SQL 규칙)
  - `insert_blocks`가 해시를 채움; 인덱서가 alloy `block.header`에서 hash/parent_hash 추출
  - 기존 범위 인덱싱 회귀 없음(NULL 백필 허용)
- *Artifacts*: `migrations/<ts>_add_block_hashes.sql`, `crates/db/src/models.rs`(Block +2),
  `crates/db/src/queries.rs`(insert_blocks UNNEST 확장), `crates/indexer/src/worker.rs`
  (Block 빌드 시 해시)
- *Decision*: D010 — reorg 감지 위해 block_hash/parent_hash 저장(대안: RPC 매 폴링
  재조회 → 비용↑). 마이그레이션 채택. (S06 착수 시 DECISIONS 기록)

## T02 — 순수 fork 탐지

**Must-haves**
- *Truths*
  - `find_fork_point(local: &[(u64, String)] /* (height, hash) desc */,
    chain_hash_at: impl Fn(u64)->Option<String>) -> Option<u64>`: 로컬과 체인 해시가
    어긋나는 최댓높이 탐색, 일치하면 `None`. 순수 → 단위테스트(체인 조회는 클로저 주입)
  - 경계: 로컬 비어있음 → None; 전부 일치 → None; tip만 불일치 → tip
- *Artifacts*: `crates/indexer/src/worker.rs`(`find_fork_point` + `#[cfg(test)]`),
  최근 N블록 로컬 해시 조회 `db::queries`
- *Key Links*: S05 `next_target` 분리 패턴 그대로 (RPC 없는 단위테스트)

## T03 — 멱등 롤백 + 재인덱싱

**Must-haves**
- *Truths*
  - fork point `f` 발견 시: `block`/`transaction`/이벤트·trace·failed 중 `block_number >= f`
    행 삭제(FK 완화 상태 → **명시적 삭제 순서** 또는 ON DELETE CASCADE 확인),
    `indexer_checkpoint` = `f-1`, 이후 follow가 자연 재인덱싱
  - **멱등**: 같은 정정 두 번 적용해도 중복/유령 행 없음(통합테스트로 단언)
- *Artifacts*: `db::queries::rollback_from_block(f)`(트랜잭션 1개), 통합테스트
  (`#[ignore]`: 시드 → 가짜 fork → rollback → 상태 검증)
- *Key Links*: 기존 삭제 대상 테이블 스키마, sqlx 트랜잭션

## T04 — follow 통합 + 검증/문서 + Reassess

**Must-haves**
- *Truths*: follow 루프가 매 폴링마다 fork 체크 → 있으면 rollback 후 진행.
  `cargo test`(순수+통합) green, clippy/fmt, prod unwrap 0
- *Artifacts*: worker `follow` 확장, `docs/realtime-follow.md` reorg 절 추가,
  `.gsd/S06-SUMMARY.md`, ROADMAP S06 `[x]`, S07 분해
- *Reassess*: S07(`eth_subscribe`/관측성) `[sketch]` 해제·분해

---

## Slice 수용 (Complete 게이트)
- [ ] T01–T04 must-haves, 마이그레이션 멱등, 기존 인덱싱 무회귀
- [ ] 순수 fork 단위테스트 + 멱등 rollback 통합테스트 green, clippy/fmt
- [ ] 라이브 reorg 수동 스모크 절차 문서화(RPC 부재 시 테스트가 1차 증빙)
- [ ] D010 기록, S07 분해
