---
slice: S16
title: 인증 미들웨어 + 보호 게이트 (M006 first slice)
status: done
edge: untapped — 운영 게이트
milestone: M006
tasks: [T01, T02]
gate: pass             # fmt clean · clippy --workspace --all-targets -D warnings 0 · -p api 단위 13/13 + 통합 7/7 · -p indexer 36/36 · -p db --lib 17/17 · -p db --ignored 27/27 · -p decoder 18/18 · web typecheck/test 29/build 900+ modules
decisions: [D021, D022, D023]
artifacts:
  - Cargo.toml + crates/api/Cargo.toml      # subtle.workspace 추가, tower dev-dep
  - crates/api/src/lib.rs                   # 신설 — 모든 모듈 pub re-export (통합테스트가 외부 입장에서 router 빌드 가능)
  - crates/api/src/auth.rs                  # 신설 — AdminAuth extractor + 단위테스트 7 (subtle::ConstantTimeEq)
  - crates/api/src/config.rs                # admin_api_key + from_env_with(get) + Debug 마스킹 + 단위테스트 6
  - crates/api/src/error.rs                 # ApiError::Unauthorized + 401 매핑(고정 메시지)
  - crates/api/src/routes/state.rs          # 신설 — ApiState + FromRef<ApiState> for PgPool
  - crates/api/src/routes/mod.rs            # build_router(state: ApiState) + 보호 표 doc
  - crates/api/src/routes/contract_labels.rs # 2 핸들러 _: AdminAuth (POST/DELETE)
  - crates/api/src/routes/alerts.rs         # 3 핸들러 _: AdminAuth (POST/DELETE/rotate)
  - crates/api/src/main.rs                  # lib import + ApiState 조립
  - crates/api/tests/auth.rs                # 신설 — tower oneshot 통합테스트 7 (보호 5 + 비보호 2)
  - .env.example                            # AMARILLO_ADMIN_API_KEY placeholder + 정책 주석
  - docker-compose.yml                      # api 서비스 environment에 ${VAR:?required} 박음
  - crates/decoder/src/events.rs            # 별 단위 — toolchain 회귀 (test, allow(cmp_owned))
  - crates/indexer/src/worker.rs            # 별 단위 — toolchain 회귀 (test, &chain → chain)
verification_constraint: "통합테스트는 PgPool::connect_lazy로 DB 미터치 — extractor가 401을 핸들러 진입 *전에* 거름. 정상 200/201/204 시나리오는 verify 스크립트(S17, docker compose) 책임. verify-alerts.sh + verify-failed-tx-by-label.sh는 본 슬라이스 출하 시점 깨짐(보호 라우트 호출 시 401) — S17에서 일괄 인증 헤더 추가로 해소가 명시적 의존."
---

# S16 — 무엇이 실제로 일어났나

M006 첫 슬라이스. **API key 인증 인프라(T01) + 보호 라우트 5개 게이트(T02)**.
M005까지의 "데모 스코프 인증 미부착"(D008/D013/D019) 정직성을 *운영 게이트*로
마감하는 첫 호흡. **(A) API key Bearer + (X) write/admin만 보호 + (1) env 단일
키** 묶음(D021).

## 응답·표면 — 운영자 완결 흐름 (S17/S18 의존)

| 단계 | 상태 | 출하 위치 |
|------|------|-----------|
| 인증 게이트 인프라 (AdminAuth extractor / ApiError::Unauthorized / ApiConfig 마스킹) | ✅ | **S16-T01** |
| 보호 라우트 5개 핸들러 시그니처에 게이트 부착 | ✅ | **S16-T02** |
| 401 통합테스트(routing layer) | ✅ | **S16-T02** |
| verify 스크립트 3종 인증 헤더 + 401 case | ⏳ | S17 의존 |
| examples (TS/Python) `apiKey` 옵션 가산 | ⏳ | S17 의존 |
| cookbook 인증 헤더 + 4 시나리오 갱신 | ⏳ | S17 의존 |
| docs/api-failed-tx.md "Authentication" 섹션 | ⏳ | S17 의존 |
| 프론트 `/alerts` 페이지 키 입력 UI | ⏳ | S18 의존 |

## 수용 기준 (REQUIREMENTS.md#M006 S16) — 항목별 ✅

| 기준 | 상태 | 증빙 |
|------|------|------|
| `AMARILLO_ADMIN_API_KEY` 미설정 또는 빈 → 부팅 실패 (silent default 금지) | ✅ | `ApiConfig::from_env_with`에서 거부, 단위테스트 `from_env_with_missing_admin_key_fails` + `from_env_with_empty_admin_key_fails` |
| 길이 < 32 → WARN 로깅 (거부 X) | ✅ | `tracing::warn!` + `from_env_with_short_admin_key_succeeds` 단위테스트 |
| `AdminAuth` extractor — 헤더 누락/형식 오류/키 불일치 모두 401 (info-leak 방지 단일 응답) | ✅ | 단위테스트 7 (missing/Basic/no-prefix/empty-bearer/wrong-key/correct-key/non-ASCII) + 통합테스트 5 case loop |
| 키 비교는 상수시간 | ✅ | `subtle::ConstantTimeEq::ct_eq` 사용 (코드 + doc comment) |
| 보호 대상 5개 (POST/DELETE `/v1/contract-labels`, POST/DELETE/rotate `/v1/alert-subscriptions`) | ✅ | 핸들러 시그니처에 `_: AdminAuth` 박힘, 통합테스트가 5개 모두 401 단언 |
| 키는 로그에 미노출 (`ApiConfig` Debug 마스킹) | ✅ | `debug_masks_admin_api_key_and_database_url` 단위테스트 (`super-secret-key` + `secret123`(DATABASE_URL password) 둘 다 미노출 단언) |
| `ApiError::Unauthorized` variant + 통합테스트 | ✅ | `crates/api/tests/auth.rs` 7 통합테스트 (보호 5 + 비보호 2 스모크) |

## 최종 게이트 (2026-05-21, 단일 호흡 재실행 — KNOWLEDGE S04 Rule)

- `cargo fmt --check` (workspace) — clean
- `cargo clippy --workspace --all-targets -- -D warnings` — 0
- `cargo test -p api` — **단위 13/13 + 통합 7/7** = 20/20
- `cargo test -p indexer` — **36/36** 무회귀
- `cargo test -p db --lib` — **17/17** 무회귀
- `cargo test -p db -- --ignored` — **27/27** 무회귀 (3 alerts + 3 alert_rate + 3 category_diagnosis + 10 failed_tx + 4 function_signature + 3 labels + 1 rollback)
- `cargo test -p decoder` — **18/18** 무회귀
- `cd web && npm run typecheck && npm run test && npm run build` — clean / **29/29** / 900+ modules (코드 변경 0 → 무회귀 자동)

## 태스크

- **T01** 인프라 (`auth.rs` extractor + `config.rs` 마스킹 + `error.rs` Unauthorized + `routes/state.rs` ApiState + main.rs state 조립 + `.env.example` / docker-compose.yml) — D021/D022/D023 모두 본 슬라이스 진입 전 기록.
- **T02** 게이트 부착 (`contract_labels.rs` 2 + `alerts.rs` 3 핸들러 시그니처 + `routes/mod.rs` 보호 표 doc + `crates/api/src/lib.rs` 신설 + 통합테스트 7).

## 핵심 교훈 (KNOWLEDGE 후보)

- **info-leak 방지 단일 401 응답 (D021)** — 헤더 누락 / Bearer 형식 오류 / 키 불일치 / 길이 불일치 모두 *동일한 401* `{"error":"unauthorized"}`. "키가 있는지" / "길이가 맞는지" 어느 것도 응답에 노출 X. 클라이언트 사이드 디버깅이 어렵다는 *대가*가 있으나 timing/oracle 공격 표면을 최소화. JWT 등 토큰 검증 표준 패턴 일관.
- **전역 layer 대신 핸들러별 extractor (D022)** — 보호 라우트의 핸들러 시그니처에 `_: AdminAuth`가 *반드시* 박혀야 컴파일됨 → 새 write/admin 라우트 추가 시 layer 등록 깜빡 회귀를 **컴파일 시점에 차단**. 트레이드오프: "어떤 라우트가 보호되는가"가 라우터 트리에 한눈에 안 보임 — routes/mod.rs doc comment의 *보호 표* + `crates/api/tests/auth.rs`의 *401 case 표*가 보완.
- **getter 주입 패턴(`from_env_with`)** — `env::var`를 직접 호출하는 `from_env`를 *공식 API*로 두고 thin wrapper로 만든 뒤, 테스트는 `from_env_with(fake_getter)`로 호출 → 글로벌 env 상태 mutation 없이 모든 분기 검증. `set_var`/`remove_var` 직렬화 필요 회피 + Rust 2024 edition의 `env::set_var` unsafe 트랜지션 회피.
- **silent default 금지 + 길이는 권고만(D023)** — 빈/미설정은 *부팅 실패*(D004 정신, 운영 실수 차단). 짧은 키는 *WARN*만 — 데모 환경에서 부팅 막힘 회피와 운영 가드의 균형. *형식 강제는 운영자 부담, 보안 이득 0*(엔트로피만 중요).
- **binary crate → lib crate 후속 변경 패턴** — `crates/api`는 binary-only였으나 통합테스트(`crates/api/tests/auth.rs`)가 외부 입장에서 모듈 import 필요 → `src/lib.rs` 신설 + `main.rs`는 lib import로 단순화. PLAN에 명시 안 했지만 *자연 follow*인 표준 Rust 패턴. main.rs 동작 무영향.
- **PgPool::connect_lazy로 통합테스트의 DB 미터치** — extractor가 *핸들러 진입 전*에 401을 거름 → 핸들러 코드(DB 호출 포함)는 절대 실행되지 않음. lazy pool은 connect 시도조차 안 함 → 통합테스트가 *환경 무의존*하게 라우팅+인증 레이어만 검증 가능. 정상 200/201/204 검증은 verify 스크립트(S17, docker compose)로 위임.

## 정직한 한계 / 잔여

- **verify-alerts.sh + verify-failed-tx-by-label.sh 깨짐** — 보호 라우트 호출 시 401. 본 슬라이스 출하 시점에 **S17 명시적 의존**. verify-failed-tx.sh는 공개 GET만 호출이라 무회귀 자동(검증 미실행 — 서버 띄움이 S17 책임).
- **examples (TS/Python) 깨짐** — `createContractLabel` / `deleteContractLabel` / alert subscription 메서드 호출 시 401. S17에서 `AmarilloClient` 생성자에 `apiKey?` 옵션 가산.
- **프론트 `/alerts` 페이지 깨짐** — write 버튼이 401. S18에서 키 입력 UI 도입.
- **cookbook 4 시나리오 모두 인증 미표시** — S17에서 시나리오 step별 인증 헤더 명시 + 401 사례 1건.
- **`docs/api-failed-tx.md`에 "Authentication" 섹션 없음** — S17에서 추가.
- **회전 = env 갱신 + 재시작** (D021/D023 트레이드오프) — 무중단 회전은 multi-key runtime 회전, *별 슬라이스*. 키 유출 대응 절차는 cookbook (S17)에서 명시.
- **rate limiting / audit log 미부착** — 키 brute-force 방어는 별 단위. write 라우트만 보호하므로 brute-force 표면은 좁음(GET 비보호 X), 401 응답이 attacker에게 key 정보 X.
- **별 단위 hardening 후보**: `crates/decoder/src/events.rs` + `crates/indexer/src/worker.rs`의 toolchain 회귀 lint 2건(test code, rust-clippy 1.92로 새로 잡힘) — 본 슬라이스에서 최소 변경(`#[allow]` 부착 + 인라인 fix)으로 게이트 통과, 의미는 무변경. 코드 의도가 *더 명확한 표현*으로의 리팩토링은 별 단위.

## Reassess

ROADMAP **M006 S16 `[x] DONE`**. S17 sketch 해제 + 태스크 분해 박음 (verify
스크립트 3종 인증 헤더 + 401 case / examples client `apiKey` 옵션 / cookbook
4 시나리오 갱신 / docs Authentication 섹션). **S18은 sketch 유지** — S17 출하
후 해제(GSD-2: 출하 전 분해 금지 원칙 일관).

다음 호흡: **S17 진입** — 인증 인프라가 *전 흐름에 일관되게* 흐르도록 verify
스크립트·examples·cookbook·docs 일괄 갱신. S17 출하로 *운영자가 실제로 사용
가능*한 상태에 도달(verify ALL PASS, examples가 동작, cookbook 따라 하면
end-to-end 흐름).
