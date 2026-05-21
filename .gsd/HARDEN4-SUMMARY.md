---
slice: HARDEN4
title: toolchain 회귀 lint 정리 — decoder cmp_owned named binding
status: done
edge: 코드 품질
milestone: standalone (BACKLOG 별 단위)
tasks: [T01]
gate: pass             # fmt clean · clippy --workspace --all-targets -D warnings 0 · -p decoder 31/31 · -p api 단위 13 + 통합 7 = 20/20 · -p indexer 36/36 · -p db --lib 17/17 무회귀
decisions: 없음 (코드 품질 리팩토링, 결정 사항 없음)
artifacts:
  - crates/decoder/src/events.rs   # test_decode_swap_event — #[allow(clippy::cmp_owned)] 제거 + named binding (zero_in/zero_out) 패턴
verification_constraint: "코드 변경은 단위테스트 코드 1군데만 — 의미 무변경(여전히 amount_in/amount_out > 0 단언). 라이브 영향 없음(test code), 게이트는 clippy + 무회귀 단위테스트로 충분."
---

# HARDEN4 — 무엇이 실제로 일어났나

BACKLOG #3 (작은 hardening, 별 단위 PR). S16에서 cargo clippy 1.92 회귀로
잡힌 `cmp_owned` lint를 *인라인 fix*(`#[allow(clippy::cmp_owned)]` + 메모)로
게이트 통과했었음 — 본 슬라이스가 *깔끔한 named binding 리팩토링*으로 allow
제거 + 의도 명확화.

## 변경 한 곳

`crates/decoder/src/events.rs::test_decode_swap_event` (test code, line 349):

**Before** (S16 인라인 fix):
```rust
#[test]
#[allow(clippy::cmp_owned)]
// S16/M006 게이트 통과 시점 toolchain (rust-clippy 1.92)에서 ...
fn test_decode_swap_event() {
    ...
    assert!(s.amount_in > BigDecimal::from(0));
    assert!(s.amount_out > BigDecimal::from(0));
}
```

**After** (HARDEN4 리팩토링):
```rust
#[test]
fn test_decode_swap_event() {
    ...
    // HARDEN4: assert "positive" without creating a throwaway BigDecimal on
    // each comparison ... Two named bindings — the comparison takes
    // ownership, and BigDecimal isn't Copy, so we declare one per assertion
    // rather than `.clone()`-ing implicitly.
    let zero_in = BigDecimal::from(0);
    let zero_out = BigDecimal::from(0);
    assert!(s.amount_in > zero_in);
    assert!(s.amount_out > zero_out);
}
```

`#[allow(clippy::cmp_owned)]` 어트리뷰트 제거. 의미 변경 없음(여전히 양수 단언).
두 binding은 *BigDecimal: !Copy*라 첫 단언이 ownership을 가져간 후 두 번째에
새 인스턴스 필요 — `.clone()` 대신 *명시적 두 binding*이 의도 명확.

## 게이트

- `cargo fmt --check` (workspace) — clean
- `cargo clippy --workspace --all-targets -- -D warnings` — **0** (이전엔 allow
  로 우회, 이제 *정직하게 통과*)
- `cargo test -p decoder` — **31/31** (test_decode_swap_event 포함, 무회귀)
- `cargo test -p api` — 단위 13 + 통합 7 = **20/20** 무회귀
- `cargo test -p indexer` — **36/36** 무회귀
- `cargo test -p db --lib` — **17/17** 무회귀

## 핵심 교훈

- **inline `#[allow(lint)]` → named binding 리팩토링** — toolchain 회귀로
  잡힌 lint는 *임시 allow*로 게이트 통과 + *별 슬라이스 후속 정리* 패턴이
  깔끔. allow가 *영구히 남으면* lint의 의도(여기선 "owned 임시 인스턴스 비교
  피하기")가 *암묵적 거부*가 되어 코드 의도가 흐려짐.
- **BigDecimal !Copy의 함의** — 비교 연산자 `>`는 `PartialOrd<&Self>` 호출
  이지만 *bare value 사용*은 ownership을 가져감. 같은 binding 두 번 사용
  불가 → 별 binding 두 개 또는 `clone()`. 본 fix는 *명시적 binding 두 개*로
  의도 (각 단언이 *독립적 비교*) 표현.
- **toolchain 회귀의 *별 단위 hardening* 정신** — 인라인 fix(S16)에서 *기능
  슬라이스의 게이트만 보장* + 별 슬라이스(HARDEN4)에서 *코드 품질 마무리*.
  GSD-2의 "한 슬라이스 한 컨텍스트" 일관 — 기능과 lint 정리를 *시간적으로
  분리*하면 두 작업 모두 *명확*.

## 정직한 한계

- **단일 위치 fix** — `crates/indexer/src/worker.rs`의 `needless_borrows`
  fix는 S16에서 이미 *깔끔하게 인라인*(`&chain` → `chain`)이라 본 슬라이스
  스코프 X. 두 회귀 중 *진짜 정리 필요*한 1건만.
- **lint allow의 *합법적 사용*은 별 이슈** — `#[allow(dead_code)]` (S16
  AdminAuth 임시) 같은 *의도적 allow*는 본 슬라이스 스코프 X. 본 작업은
  *toolchain 회귀로 잡힌 lint*만 정리.
- **라이브 영향 없음** — test code 1군데, 의미 무변경. 운영자/사용자 영향
  0.

## Reassess

ROADMAP 완료 백로그 표에 `HARDEN4 — toolchain 회귀 lint 정리` 추가. BACKLOG.md
"별 단위 hardening" 항목 제거 + 우선순위 표 재정렬.

다음 호흡은 사용자 결정 — S13.1 / hickory-dns / FE / M007 분기 중. BACKLOG
우선순위 표 활용.
