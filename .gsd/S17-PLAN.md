# S17 — verify 스크립트 + examples 클라이언트 + cookbook 인증 헤더 · PLAN

Slice 목표: REQUIREMENTS#M006 두 번째 슬라이스 — S16에서 박힌 인증 게이트가
*전 흐름에 일관되게* 흐르도록 검증 스크립트 3종 + examples 클라이언트(TS/Python) +
cookbook 4 시나리오 + docs Authentication 섹션을 갱신. 본 슬라이스 출하 시점에
*운영자가 실제로 사용 가능*한 상태에 도달(verify ALL PASS, examples 동작,
cookbook 따라 하면 end-to-end 흐름).

엣지: `[edge: weak-spot — 운영 안전 표면]`. risk: low. deps: M001~M005 + S16 인증
인프라.

핵심 결정: 본 슬라이스 진입 전 결정 완료 — **D021/D022/D023** 일관(S16과 동일).
새 결정 없음. *호환성 패턴* 정리:
- examples 클라이언트 생성자 — 옵션 가산 (기존 `new AmarilloClient(baseUrl)` 호환 +
  새 옵션 `apiKey?` 또는 options object). Python은 `api_key=None` keyword arg로
  자연 호환.
- write 메서드만 자동 `Authorization` 헤더 부착 — GET 무부착 (D021/X 정책 일관,
  임베드성 보존). 단, 사용자가 명시적으로 키를 박으면 GET에도 부착 가능한 옵션은
  *별 슬라이스* (현재 요구 없음).

검증 제약(D009~D023 일관): T01 verify 3종 ALL PASS (docker compose + 키 설정),
T02 examples `tsc --noEmit` clean + `py_compile` clean (라이브 호출 검증은
verify 스크립트로 우회), T03 cookbook + docs 마크다운 (빌드 영향 없음).

태스크: T01 → T02 → T03.

---

## T01 — verify 스크립트 3종 인증 헤더 + 401 case

**Must-haves**
- *Truths*
  - `scripts/verify-failed-tx.sh`:
    - **env 필수 추가**: `: "${AMARILLO_ADMIN_API_KEY:?required (M006)}"` —
      미설정 즉시 실패. 서버 부팅에 필수이므로 verify 스크립트도 강제 받음.
    - 서버 환경에 키 export 추가 (`export … AMARILLO_ADMIN_API_KEY`).
    - curl 호출은 *현재처럼 GET만* — 보호 라우트 호출 없음. 헤더 부착 X
      (임베드 사용처와 정합, D021/X 정책 일관).
  - `scripts/verify-alerts.sh`:
    - env 강제 + export 동일.
    - 보호 라우트 호출에 `-H "Authorization: Bearer ${AMARILLO_ADMIN_API_KEY}"`
      추가:
      - `POST /v1/alert-subscriptions` (per_event + rate_threshold 모두)
      - `DELETE /v1/alert-subscriptions/{id}`
      - `POST /v1/alert-subscriptions/{id}/rotate-secret`
    - **신규 401 case** (스크립트 하단 새 절):
      - 키 누락(헤더 X) → 401
      - 잘못된 키(`Bearer wrong-key-xxxx`) → 401
      - 두 케이스 모두 응답 body가 `{"error":"unauthorized"}` 단언.
    - 기존 시나리오(S08 per_event / S14 rate / 400 거부 etc) 무회귀.
  - `scripts/verify-failed-tx-by-label.sh`:
    - env 강제 + export 동일.
    - 보호 라우트 호출에 헤더 추가:
      - `POST /v1/contract-labels` (UPSERT 시연 두 번 + invalid 거부)
      - `DELETE /v1/contract-labels/{address}` (round-trip)
    - **신규 401 case**: 키 누락 + 잘못된 키 (위와 동일 패턴).
    - 기존 시나리오(by-label GET / S15 admin / 400 거부) 무회귀.
  - 모든 스크립트의 `set -uo pipefail` + cleanup trap 유지 — 401 case 추가가
    조기 종료를 일으키지 않도록 (`curl` 401 응답은 *예상된 결과*라 fail로
    카운트되면 안 됨; status code 단언으로 검증).
- *Artifacts*: `scripts/{verify-failed-tx,verify-alerts,verify-failed-tx-by-label}.sh`
- *Key Links*: S16 `AdminAuth` extractor (401 단일 응답 정책), S08 verify-alerts
  의 기존 status 단언 패턴, S15 verify-failed-tx-by-label의 POST/DELETE round-trip

## T02 — examples 클라이언트(TS/Python) `apiKey` 옵션 가산

**Must-haves**
- *Truths*
  - `examples/typescript-client/client.ts`:
    - `AmarilloClient` 생성자 시그니처 갱신 — 옵션 추가 패턴:
      - **방향 A** (호환): `constructor(baseUrl: string, options?: { apiKey?: string })`
        — 기존 사용자(`new AmarilloClient(baseUrl)`)는 무변경. 새 옵션은 두 번째 인자.
      - **방향 B** (options object): `constructor(options: { baseUrl: string; apiKey?: string })`
        — 기존 사용자 갱신 필요(`new AmarilloClient({ baseUrl })`).
      - **결정**: 방향 A (호환 우선, D017 정신 — 카피해서 쓰는 사용자가
        절대 깨지지 않음). cookbook 예시도 두 인자.
    - private `apiKey?: string` 필드 저장.
    - `private _request` 헬퍼에 *write 메서드만* `Authorization: Bearer ${this.apiKey}`
      자동 부착. 헬퍼 시그니처 확장: `_request(path, init?, { auth?: 'admin' } = {})`
      — `auth: 'admin'`이면 헤더 부착 + 키 없으면 *클라이언트 측 throw*
      (서버 401 받기 전 사전 차단, 운영자에게 명확한 신호).
    - write 메서드 5개에 `auth: 'admin'` 박음:
      `createAlertSubscription` / `deactivateAlertSubscription` /
      `rotateAlertSubscriptionSecret` / `createContractLabel` /
      `deleteContractLabel`.
    - GET 메서드는 무변경 (D021/X 정책).
    - `tsc --noEmit -p examples/typescript-client/tsconfig.json` clean.
  - `examples/python-client/client.py`:
    - `AmarilloClient.__init__(self, base_url: str, api_key: Optional[str] = None)` —
      keyword arg로 자연 호환.
    - `self.api_key: Optional[str]` 저장.
    - `_request` 헬퍼에 `auth: bool = False` 인자 추가 — `True`면 헤더
      부착 + 키 없으면 `ValueError` raise (사전 차단).
    - write 메서드 5개에 `auth=True` 박음 (TS와 동일 5개).
    - `python3 -m py_compile examples/python-client/{client,examples}.py` clean.
  - examples 자체에 *401 시나리오 테스트는 미추가* — verify 스크립트가
    HTTP 레벨에서 검증. examples는 *호출 방법 시연* 책임(D017 정신).
- *Artifacts*: `examples/typescript-client/client.ts`,
  `examples/python-client/client.py`
- *Key Links*: S13 examples 외부 의존 0 정책(D017), EXAMPLES-ADMIN의
  `createContractLabel` / `deleteContractLabel` 메서드 패턴

## T03 — cookbook 4 시나리오 + docs Authentication 섹션 + S17-SUMMARY + ROADMAP

**Must-haves**
- *Truths*
  - `docs/cookbook.md` 4 시나리오 점검 — *write 호출이 있는 시나리오*에 인증
    헤더 명시:
    - **시나리오 1** (단건 진단) — GET 위주, 무영향. *글로벌 안내 문단*에서
      "write/admin은 Authorization 헤더 필요, GET은 무관" 한 줄 추가.
    - **시나리오 2** (알림 구독) — `POST /v1/alert-subscriptions`이 보호:
      curl + TS + Python 모두 `Authorization: Bearer` 명시. TS는
      `new AmarilloClient(baseUrl, { apiKey })`, Python은
      `AmarilloClient(base_url, api_key=...)` 시그니처 갱신.
    - **시나리오 3** (라벨 분포) — GET `/v1/analytics/failed-tx/by-label`은
      무보호, 단 *라벨 등록 사전 step*이 POST `/v1/contract-labels`(보호).
      현재 시나리오에 그 사전 step이 있다면 갱신; 없다면 시나리오 4 참조.
    - **시나리오 4** (봇 운영자) — step 1 (라벨 등록 POST) + step 2 (rate
      sub 생성 POST) 모두 보호. curl + TS + Python 헤더 부착, **401 사례
      1건** 추가 ("키 누락 시" 한 단락).
  - `docs/api-failed-tx.md` **"Authentication" 섹션 신설** (목차에 추가):
    - 환경 변수: `AMARILLO_ADMIN_API_KEY` (필수, 32+ bytes 권고)
    - 보호 대상 표 (5 라우트, S16-PLAN 표 재사용)
    - 401 응답 형식: `{"error":"unauthorized"}` + status 401
    - curl 정상 예시 + 401 예시
    - 회전 절차: env 갱신 + 서버 재시작 (D021/D023 트레이드오프 명시)
    - **D008/D013/D019의 "데모 스코프 인증 미부착" 메모 제거 또는 갱신** —
      관련 absatz가 S15 admin endpoint 절에 있을 가능성. 새 정책 반영.
  - `.gsd/S17-SUMMARY.md`: T01/T02/T03 산출, 게이트 evidence, 정직한 한계
    (라이브 메인넷 자동 회귀 부재 / 프론트 `/alerts`는 *아직 깨짐 — S18 의존*).
  - `.gsd/M001-ROADMAP.md`:
    - M006 섹션 S17 → `[x] DONE → S17-SUMMARY.md`
    - S18 `[sketch]` 해제 → 태스크 분해 박기 (키 입력 UI / 401 처리 / 키 미설정
      시 write 버튼 비활성 / M006 마감 cookbook)
  - 최종 게이트 재실행 (KNOWLEDGE S04 Rule):
    - `cargo fmt --check` (workspace) — 무변경 자동
    - `cargo clippy --workspace --all-targets -- -D warnings` — 무변경 자동
    - `cargo test -p api` / `-p indexer` / `-p db --lib` / `-p decoder` 무회귀
      자동(코드 0 변경 — 본 슬라이스는 shell + TS + Python + 마크다운만)
    - `bash scripts/verify-failed-tx.sh` — ALL PASS (env 키 적용)
    - `bash scripts/verify-alerts.sh` — ALL PASS (인증 헤더 + 401 case)
    - `bash scripts/verify-failed-tx-by-label.sh` — ALL PASS (인증 헤더 + 401 case)
    - `tsc --noEmit -p examples/typescript-client/tsconfig.json` clean
    - `python3 -m py_compile examples/python-client/{client,examples}.py` clean
    - `web` typecheck/test/build 무회귀 자동 (web 0 변경)
- *Reassess*: S17 ✅ DONE. S18 sketch 해제 + 분해 박음. S18 출하 = M006 마감.
- *Artifacts*: `docs/cookbook.md`, `docs/api-failed-tx.md`,
  `.gsd/{S17-SUMMARY,M001-ROADMAP}.md`

---

## Slice 수용 (Complete = S17 SHIPPED)
- [ ] T01–T03 must-haves, 기존 모든 표면 무회귀
- [ ] verify 3종 ALL PASS (docker compose + 키 설정)
- [ ] examples `tsc --noEmit` clean + `py_compile` clean
- [ ] cookbook + docs 마크다운 (CI 영향 없음)
- [ ] `cargo test -p api` 단위 20/20 + 통합 7/7 무회귀
- [ ] `cargo test -p indexer` 36 / `-p db --lib` 17 / `-p db --ignored` 27 /
      `-p decoder` 18 무회귀
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 0,
      `cargo fmt --check` clean
- [ ] `web` typecheck + test + build 무회귀 (코드 0 변경)
- [ ] REQUIREMENTS#M006 S17 항목 ✅ + S17-SUMMARY + ROADMAP M006 S17 `[x] DONE`
- [ ] S18 sketch 해제 + 태스크 분해

## 정직한 한계 (S17 출하 시점)

- **프론트 `/alerts` 페이지 여전히 깨짐** — write 버튼이 401. **S18에서 키 입력
  UI 도입**으로 해소가 명시적 의존.
- **examples는 컴파일/타입 검증까지** — 라이브 401 시뮬은 verify 스크립트로
  우회. D017 정신 일관(examples는 SDK 아닌 카피 가능 예시).
- **라이브 메인넷 자동 회귀 부재** — 모든 검증은 docker compose 시드 데이터
  기반. 메인넷 트래픽 자동 회귀는 환경 부재로 불가능 (M005 한계 일관).
- **회전 = env 갱신 + 재시작** (D021/D023) — 본 슬라이스는 절차만 docs에
  명시. 무중단 회전은 multi-key runtime 회전, *별 슬라이스*(M006 출하 이후
  후속 후보).
- **rate limiting / audit log 미부착** — 키 brute-force 방어는 별 단위.
  write 라우트만 보호하므로 brute-force 표면은 좁고, 401 응답이 attacker
  에게 key 정보 X (S16/D021).
