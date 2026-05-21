# S16 — 인증 미들웨어 + 보호 게이트 · PLAN

Slice 목표: REQUIREMENTS#M006 첫 슬라이스 — `crates/api`에 API key 인증을 박아
write/admin 표면 5개를 401 게이트. M005까지의 "데모 스코프 인증 미부착"(D008/
D013/D019) 정직성을 *운영 게이트*로 마감하는 첫 호흡. M006 = S16 ∧ S17 ∧ S18.

엣지: `[edge: untapped — 운영 게이트]`. risk: med (state 확장 + 통합테스트 패턴
신설). deps: M001~M005 + `subtle` 신규 의존(1개).

핵심 결정: **D021** (A + X + 1 묶음 — API key Bearer / write/admin만 / env 단일 키),
**D022** (전역 layer 아닌 핸들러별 extractor 게이트), **D023** (env 단일 키,
빈/미설정 = 부팅 실패, 짧은 키 = WARN 거부 X). 모두 본 슬라이스 진입 전 기록됨.

검증 제약(D009~D020 일관): T01 단위테스트(extractor 4 case + ApiConfig 4 case),
T02 통합테스트(401 4 case + 정상 4 case) + clippy/fmt, T03 verify 일부 수동
검증(보호 라우트는 S17에서 일괄 갱신 — 본 슬라이스에서는 401만 단언).

태스크: T01 → T02 → T03.

---

## T01 — `auth.rs` + `ApiConfig` 확장 + `ApiError::Unauthorized` + state 확장

**Must-haves**
- *Truths*
  - `crates/api/src/auth.rs` **신설**:
    - `pub struct AdminAuth;` (zero-sized 단위 구조체, 시그니처 마커)
    - `impl<S: Send + Sync> FromRequestParts<S> for AdminAuth` —
      `ApiState`(또는 `Arc<ApiState>`)에서 `admin_api_key` 꺼내 비교:
      - `Authorization` 헤더 누락 → `Err(ApiError::Unauthorized)`
      - `Bearer ` prefix 누락 또는 형식 오류 → 동일 401
      - prefix 제거한 토큰을 `subtle::ConstantTimeEq::ct_eq`로 비교:
        - 길이 다름 → 동일 401 (info-leak 방지 — 길이 노출 X)
        - 같으면 `Ok(AdminAuth)`
    - **로그 미노출** — 401 시 헤더/토큰 값 절대 로그 출력 X. 401 카운터만
      `tracing::warn!`로 1줄(`{"event":"unauthorized","route":"..."}` 정도).
  - `crates/api/src/config.rs` **확장**:
    - `admin_api_key: String` 필드 추가 (필수)
    - `from_env()`:
      - `env::var("AMARILLO_ADMIN_API_KEY")` → `Err(anyhow!("…required"))` if missing
      - `.trim().is_empty()` → 동일 에러 (빈 문자열 거부)
      - `len() < 32` → `tracing::warn!("…short, recommend 32+ bytes")` 후 통과
    - `Debug` derive **제거**, `impl fmt::Debug for ApiConfig` 수동 작성 —
      `admin_api_key` 필드는 `***`로 마스킹 (HARDEN2 정신 일관: `webhook_url`
      마스킹 패턴 참조).
  - `crates/api/src/error.rs` **확장**:
    - `ApiError::Unauthorized` variant (`#[error("unauthorized")]`)
    - `IntoResponse`에서 `(StatusCode::UNAUTHORIZED, json!({"error":"unauthorized"}))`
    - 메시지는 *고정 문자열* — 운영자 키 vs 사용자 키 vs 잘못된 헤더 모두 동일
      문자열(D021 info-leak 방지).
  - `crates/api/src/routes/mod.rs` **상태 확장**:
    - `pub struct ApiState { pub db_pool: PgPool, pub admin_api_key: Arc<String> }` (또는
      `Arc<ApiState>`)
    - `build_router(state: ApiState)` 시그니처 — 기존 `db_pool: PgPool`에서 교체.
    - 모든 핸들러의 `State<PgPool>`을 `State<ApiState>`로 갱신 + `pool = &state.db_pool`
      추출 패턴 (또는 `FromRef<ApiState> for PgPool` 도입해 기존 시그니처 보존
      — *후자가 변경 최소*, 채택 권장).
  - `crates/api/src/main.rs`: `ApiConfig::from_env()` → `ApiState` 조립 → `build_router(state)`.
  - `Cargo.toml`(workspace) + `crates/api/Cargo.toml`: `subtle = { version = "2.5",
    default-features = false }` 추가 (no_std-friendly).
  - 단위테스트 (`crates/api/src/auth.rs` 내 `#[cfg(test)] mod tests`):
    - `extractor_missing_header_returns_401`
    - `extractor_malformed_bearer_returns_401` (예: `Authorization: Basic xxx`,
      `Authorization: Bearer` 빈 토큰, `Authorization: xxx` prefix 없음)
    - `extractor_wrong_key_returns_401`
    - `extractor_correct_key_returns_ok`
    - 헤더는 axum `http::Request::builder()` 또는 `axum::http::HeaderMap`로 구성
      → `AdminAuth::from_request_parts(&mut parts, &state)` 직접 호출.
  - 단위테스트 (`crates/api/src/config.rs` 내):
    - `from_env_missing_key_fails`
    - `from_env_empty_key_fails`
    - `from_env_short_key_warns_but_succeeds` (WARN 캡처는 어려움 — 길이 통과만
      단언)
    - `debug_masks_admin_api_key` (`format!("{:?}", config)`에 키 값 미노출,
      `***` 또는 마스킹 토큰 포함)
  - prod `unwrap()` 0 / `///` doc / `subtle::ConstantTimeEq` 사용 명시 doc.
- *Artifacts*: `crates/api/src/{auth.rs,config.rs,error.rs,main.rs,routes/mod.rs}`,
  `crates/api/Cargo.toml`, `Cargo.toml` (workspace)
- *Key Links*:
  - HARDEN2 — `webhook_url`/`signing_secret` 마스킹 패턴 (Debug 수동 구현)
  - S08 `crates/api/src/error.rs` — `ApiError` variant 추가 패턴
  - S08 `routes/mod.rs` — `State<PgPool>` 핸들러 시그니처 (FromRef로 보존 가능)

## T02 — 보호 라우트 5개 게이트 + 통합테스트

**Must-haves**
- *Truths*
  - 보호 라우트 핸들러에 `_: AdminAuth` **첫 파라미터 추가** (extractor 순서:
    `_: AdminAuth` → `State` → `Path/Query/Json`):
    - `crates/api/src/routes/contract_labels.rs`:
      - `create_contract_label(_: AdminAuth, State<ApiState>, Json<CreateLabelBody>)`
      - `delete_contract_label(_: AdminAuth, State<ApiState>, Path<String>)`
    - `crates/api/src/routes/alerts.rs`:
      - `create_alert_subscription(_: AdminAuth, State<ApiState>, Json<CreateBody>)`
      - `deactivate_alert_subscription(_: AdminAuth, State<ApiState>, Path<i64>)`
      - `rotate_alert_subscription_secret(_: AdminAuth, State<ApiState>, Path<i64>)`
    - 비보호 GET들은 *건드리지 않음* — `list_alert_subscriptions`,
      `failed_tx::*`, `analytics::*`, `pools::*`, `tokens::*`, `swaps::*`,
      `traders::*`, `blocks::*`, `health::*` 모두 무변경.
  - `routes/mod.rs` 보호 표 주석 — "// PROTECTED (M006/D021/D022): " 앞에 박아
    회귀 차단 가시화. 보호 표:
    ```rust
    // PROTECTED — AdminAuth extractor on handler:
    //   POST   /v1/contract-labels
    //   DELETE /v1/contract-labels/{address}
    //   POST   /v1/alert-subscriptions
    //   DELETE /v1/alert-subscriptions/{id}
    //   POST   /v1/alert-subscriptions/{id}/rotate-secret
    // (others under /v1/* and /health are public — embed-friendly)
    ```
  - 통합테스트 — `crates/api/tests/auth.rs` 신설 (또는 기존 통합테스트 패턴
    있으면 따름). 테스트 패턴:
    - axum `Router`를 직접 빌드 + `tower::ServiceExt::oneshot`으로 요청 주입
    - `ApiState` 빌드 시 `admin_api_key`는 고정 문자열 (예: `"test-key-32-bytes-long-aaaaaaaaaaa"`)
    - DB는 mock `PgPool` 어려움 → 통합테스트는 *401 검증에 집중*, 정상 200/201/204
      검증은 verify 스크립트(S17)로 이행. 401은 *extractor 게이트만* 검증하면
      되므로 DB 없이도 가능 (extractor가 핸들러 진입 전에 거름).
    - 401 4 case: 헤더 없음 / `Authorization: Basic xxx` / `Authorization: Bearer wrong-key` /
      `Authorization: Bearer ` (빈 토큰) — 5개 보호 라우트 중 *대표 2~3개* 만
      각각 4 case (조합 폭발 방지) + 나머지 라우트는 *컴파일 시그니처가 보호*
      되므로 1 case 스모크만.
  - 비보호 라우트 1개 무회귀 스모크 (예: `GET /health` 200 — 헤더 없어도).
  - DB 통합테스트 무회귀 (`-p db --ignored`).
  - prod `unwrap()` 0 / `///` doc.
- *Artifacts*: `crates/api/src/routes/{contract_labels.rs,alerts.rs,mod.rs}`,
  `crates/api/tests/auth.rs`
- *Key Links*:
  - S08 alert subscription handler 시그니처 (3 extractor 패턴)
  - S15 contract_labels handler 시그니처 (Path + Json)
  - axum `tower::ServiceExt::oneshot` 통합테스트 패턴

## T03 — S16-SUMMARY + ROADMAP S16 `[x] DONE` + S17 sketch 해제 + 최종 게이트

**Must-haves**
- *Truths*
  - `.gsd/S16-SUMMARY.md`: T01/T02 산출, 게이트 evidence, 정직한 한계
    (verify 스크립트는 *아직 무인증 — S17 dependency*, examples는 *아직 무인증
    — S17 dependency*, 프론트는 *아직 무인증 — S18 dependency*).
  - `.gsd/M001-ROADMAP.md`:
    - M006 섹션 S16 → `[x] DONE → S16-SUMMARY.md`
    - S17 `[sketch]` 해제 → 태스크 분해 박기 (verify 3종 인증 헤더 / examples
      apiKey 옵션 / cookbook 갱신 / docs Authentication 섹션)
    - S18은 sketch 유지 — S17 출하 후 해제
  - 최종 게이트 재실행 (KNOWLEDGE S04 Rule):
    - `cargo fmt --check` (workspace)
    - `cargo clippy --workspace -- -D warnings`
    - `cargo test -p api` (auth 단위 + 통합테스트)
    - `cargo test -p indexer` (무회귀)
    - `cargo test -p db --lib` (무회귀)
    - `cargo test -p db -- --ignored` (무회귀)
    - **본 슬라이스에서 verify 스크립트는 401 케이스 1건만 추가** —
      `verify-failed-tx.sh` 무회귀 + 보호 라우트에 헤더 없으면 401 단언 1건
      (다른 verify 2종은 *보호 라우트를 호출하므로 깨질 것* → 본 슬라이스
      게이트에서 *명시적으로 skipping* 또는 임시 인증 헤더 추가, S17에서
      체계화).
    - `web` 무회귀(코드 변경 없음 — typecheck/test/build 자동 통과)
  - *결정*: verify 스크립트 처리는 두 가지 방향 중 하나 선택 (PLAN에 명시):
    - **(a) S17 의존 명시 + 본 슬라이스에서는 verify 깨짐 허용** — S16-SUMMARY가
      이를 *정직한 한계*로 박고 S17이 즉시 후속 PR (M005-SUMMARY/S15 패턴 일관).
    - **(b) 본 슬라이스에서 verify 3종 모두 인증 헤더 임시 추가** — S17에서
      체계화(401 case 추가 등).
    - **추천: (a)** — S17이 *명시적 의존*. 본 슬라이스를 *작게 유지*가 GSD-2
      정신. M006 분해 의도(S16 작게, S17 verify 일괄)와 정합.
- *Reassess*: S16 ✅ DONE. S17 sketch 해제 + 태스크 분해. S18은 S17 출하 후
  분해(GSD-2: 출하 전 분해 금지 원칙 일관).
- *Artifacts*: `.gsd/{S16-SUMMARY,M001-ROADMAP}.md`

---

## Slice 수용 (Complete = S16 SHIPPED)
- [ ] T01–T03 must-haves, 기존 모든 비보호 표면 무회귀
- [ ] `cargo test -p api` 단위(extractor 4 + config 4) + 통합(401 ≥ 4 + 비보호
      스모크 1) 모두 green
- [ ] `cargo clippy --workspace -- -D warnings` 0, `cargo fmt --check` clean
- [ ] `cargo test -p indexer` + `cargo test -p db --lib` + `cargo test -p db -- --ignored`
      모두 무회귀
- [ ] verify 스크립트는 정직한 한계로 명시 (S17 의존) — `verify-failed-tx.sh`는
      *비보호 GET*만 호출하므로 무회귀, 나머지 2종은 S17에서 인증 헤더 박음
- [ ] `web` typecheck + test + build 무회귀 (코드 변경 0)
- [ ] REQUIREMENTS#M006 S16 항목 ✅ (수용 기준 5개 모두) + S16-SUMMARY + ROADMAP
      M006 S16 `[x] DONE`
- [ ] BACKLOG/ROADMAP 라벨 정합 마무리 (M006 S17 sketch 해제 박기)

## 정직한 한계 (S16 출하 시점)
- **verify 2종 깨짐**(verify-alerts / verify-failed-tx-by-label) — 보호 라우트
  호출 시 401. **S17에서 인증 헤더 일괄 추가**로 해소. 본 슬라이스 출하 PR에
  명시 + S17이 즉시 후속.
- **examples 깨짐 동일** — TS/Python 클라이언트의 admin/write 메서드 호출 시
  401. S17에서 `apiKey` 옵션 가산.
- **프론트 `/alerts` 페이지 깨짐** — write 버튼이 401. S18에서 키 입력 UI 도입.
- **DB 통합테스트는 인증과 무관** — `crates/db/tests/*` 모두 DB 쿼리만 호출
  (HTTP X) → 무회귀 자동.
- **rate limiting / audit log X** — 키 brute-force 방어는 별 단위(rate limiter
  미도입, GET 비보호이므로 brute-force 표면은 write 라우트만 — 401 응답이
  attacker에게 key 정보 X).
