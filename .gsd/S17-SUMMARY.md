---
slice: S17
title: verify 스크립트 + examples 클라이언트 + cookbook 인증 헤더 (M006 second slice)
status: done
edge: weak-spot — 운영 안전 표면
milestone: M006
tasks: [T01, T02, T03]
gate: pass             # fmt clean · clippy --workspace --all-targets -D warnings 0 · -p api 단위 13 + 통합 7 = 20/20 · -p indexer 36/36 · -p db --lib 17/17 · -p db --ignored 27/27 · -p decoder 18/18 · verify 3종 ALL PASS (포트 3005, 키 export 후) · tsc --noEmit clean · py_compile clean · web typecheck/test 29/29 무회귀
decisions: []          # 새 결정 없음; D021/D022/D023 일관
artifacts:
  - scripts/verify-failed-tx.sh             # env 강제 (GET only, 헤더 부착 X)
  - scripts/verify-alerts.sh                # 보호 POST/DELETE/rotate 헤더 + 401 case 2
  - scripts/verify-failed-tx-by-label.sh    # POST/DELETE 헤더 + 401 case 2
  - examples/typescript-client/client.ts    # ClientOptions{apiKey?} + auth: "admin" 분기 + 5 write 자동 헤더 + missing key 사전 throw
  - examples/typescript-client/examples.ts  # main에서 AMARILLO_ADMIN_API_KEY env, 키 없으면 demo #2 skip
  - examples/python-client/client.py        # api_key=None + auth=True 분기 + 5 write 자동 헤더 + missing key ValueError
  - examples/python-client/examples.py      # main에서 os.environ, 키 없으면 demo #2 skip
  - docs/cookbook.md                        # 글로벌 Authentication note + 시나리오 2/4 헤더 + step 1·2 TS/Python 키 패턴 + 401 사례 (S17 신설 절)
  - docs/api-failed-tx.md                   # 상단 "Authentication" 종합 섹션(보호 표 5 + 401 + curl + 회전 절차) + 기존 "Auth (or the lack of it)" 갱신
verification_constraint: "verify 3종은 docker compose postgres + AMARILLO_ADMIN_API_KEY env 설정 후 ALL PASS. 라이브 메인넷 자동 회귀는 환경 부재로 불가능 (M004/M005 일관 한계). 프론트 /alerts 페이지는 여전히 401 — S18 의존."
---

# S17 — 무엇이 실제로 일어났나

M006 두 번째 슬라이스. S16에서 박힌 인증 게이트가 *전 흐름에 일관되게* 흐르도록
**verify 스크립트 3종 + examples 클라이언트(TS/Python) + cookbook + docs 갱신**.
본 슬라이스 출하 시점에 운영자가 *실제로 사용 가능*한 상태에 도달 — verify
3종 ALL PASS, examples가 키와 함께 동작, cookbook 따라 하면 end-to-end 흐름.

새 결정 없음 — D021(A+X+1)/D022(extractor 게이트)/D023(env 단일 키) 일관 적용.

## 응답·표면 — 인증 흐름 전 통과

| 단계 | 상태 | 출하 위치 |
|------|------|-----------|
| 인증 게이트 인프라 | ✅ | S16-T01 |
| 보호 라우트 5개 핸들러 시그니처에 게이트 | ✅ | S16-T02 |
| 401 통합테스트 | ✅ | S16-T02 |
| **verify 스크립트 3종 인증 헤더 + 401 case** | ✅ | **S17-T01** |
| **examples (TS/Python) apiKey 옵션 + 사전 throw** | ✅ | **S17-T02** |
| **cookbook 4 시나리오 인증 헤더 + 401 사례** | ✅ | **S17-T03** |
| **docs/api-failed-tx.md "Authentication" 종합 섹션** | ✅ | **S17-T03** |
| 프론트 `/alerts` 페이지 키 입력 UI | ⏳ | S18 의존 |

## 수용 기준 (REQUIREMENTS.md#M006 S17) — 항목별 ✅

| 기준 | 상태 | 증빙 |
|------|------|------|
| verify 3종 모두 `AMARILLO_ADMIN_API_KEY` env 강제 (미설정 즉시 실패) | ✅ | `: "${VAR:?required (S16/M006)}"` 패턴, env 누락 시 stderr + 비-0 exit |
| verify 3종 보호 라우트 호출에 `Authorization: Bearer` 헤더 + 401 case | ✅ | verify-alerts.sh + verify-failed-tx-by-label.sh `ALL PASS` (401 case 2건 각 포함) |
| examples TS `new AmarilloClient(baseUrl, { apiKey? })` — write 자동 헤더, GET 무부착 | ✅ | `tsc --noEmit` clean, `auth: "admin"` 5 write 메서드에 부착 |
| examples Python `AmarilloClient(base_url, api_key=None)` 동일 패턴 | ✅ | `py_compile` clean, `auth=True` 5 write 메서드 |
| 키 없이 write 호출 → *클라이언트 측* throw/raise | ✅ | TS `AmarilloError(0, "missing API key…")`, Python `ValueError` |
| cookbook 4 시나리오 인증 헤더 명시 + 401 사례 1건 | ✅ | docs/cookbook.md 상단 글로벌 안내 + 시나리오 2/4 curl·TS·Python 갱신 + "If you forget the API key" 신설 절 |
| `docs/api-failed-tx.md`에 "Authentication" 섹션 신설 | ✅ | 문서 상단 `## Authentication` (env 정책 + 보호 표 5 + 401 응답 + curl 정상/실패 + 회전 절차 + JWT/OAuth 미선택 이유) |
| 기존 "Auth (or the lack of it)" 갱신 | ✅ | 위 섹션 링크 + S16에서 닫혔다는 정직 메모 |
| 비기능: 모든 게이트 무회귀, 신규 코드 없음(shell/TS/Python/md만 변경) | ✅ | clippy/fmt/api/indexer/db lib/db ignored/decoder ALL PASS, web 0 변경 자동 무회귀 |

## 최종 게이트 (2026-05-21, 단일 호흡 재실행 — KNOWLEDGE S04 Rule)

- `cargo fmt --check` (workspace) — clean
- `cargo clippy --workspace --all-targets -- -D warnings` — 0
- `cargo test -p api` — 단위 13/13 + 통합 7/7 = **20/20** 무회귀
- `cargo test -p indexer` — **36/36** 무회귀
- `cargo test -p db --lib` — **17/17** 무회귀
- `cargo test -p db -- --ignored` — **27/27** 무회귀 (3 alerts + 3 alert_rate + 3 category_diagnosis + 10 failed_tx + 4 function_signature + 3 labels + 1 rollback)
- `cargo test -p decoder` — **18/18** 무회귀
- `bash scripts/verify-failed-tx.sh` — **ALL PASS** (env 강제 적용)
- `bash scripts/verify-alerts.sh` — **ALL PASS** (인증 헤더 + 401 case 2건)
- `bash scripts/verify-failed-tx-by-label.sh` — **ALL PASS** (인증 헤더 + 401 case 2건)
- `tsc --noEmit -p examples/typescript-client/tsconfig.json` — clean
- `python3 -m py_compile examples/python-client/{client,examples}.py` — clean
- `cd web && npm run typecheck && npm run test` — clean / **29/29** 무회귀
- 검증 환경: docker compose postgres + `AMARILLO_ADMIN_API_KEY=test-key-32-bytes-long-aaaaaaaaaa` + `API_PORT=3005` (기본 3001은 외부 프로세스 점유)

## 태스크

- **T01** verify 스크립트 3종 — env 강제(부팅 게이트 일관) + `AUTH` 변수 + 보호
  POST/DELETE/rotate `-H "$AUTH"` 적용 + 401 case 2건씩(키 누락 / 잘못된 키 모두
  `{"error":"unauthorized"}` 단언). cleanup curl도 헤더 추가(보호 라우트라).
- **T02** examples 클라이언트 — *방향 A* (호환 우선) 채택. TS 두 번째 인자
  `ClientOptions`, Python `api_key` keyword. `_request` 헬퍼에 `auth` 옵션
  추가 — admin 라우트만 헤더 부착 + 키 없으면 *서버에 보내기 전* throw/raise.
  examples.py / examples.ts main이 `AMARILLO_ADMIN_API_KEY` 환경에서 키 읽고,
  키 없으면 demo #2(alert subscription) skip 안내.
- **T03** cookbook + docs + SUMMARY — 글로벌 Authentication note(상단), 시나리오
  2 curl create/rotate 헤더, 시나리오 4 step 1/2 curl·TS·Python 헤더 + 키 패턴,
  새 절 "If you forget the API key" (401 사례). docs/api-failed-tx.md 상단에
  `## Authentication` 종합 섹션 신설 (env 정책 + 보호 표 5개 + 401 응답 + curl
  예시 + 회전 절차 + JWT/OAuth 미선택 이유). 기존 "Auth (or the lack of it)"는
  새 섹션 링크 + S16 마감 메모.

## 핵심 교훈 (KNOWLEDGE 후보)

- **호환 우선 옵션 패턴 (방향 A)** — `new AmarilloClient(baseUrl, options?)`는
  기존 카피해서 쓰는 사용자(D017 정신)가 *절대 깨지지 않음*. options-object
  방향 B는 더 깔끔하나 *기존 호출 모두 갱신 필요* → 카피 가능 예시의 안정성
  희생. cookbook이 두 패턴 다 시연했으면 *혼동*만 가산. *한 패턴*을 선택하고
  cookbook도 동일하게 — *일관성 > 유연성*.
- **클라이언트 측 사전 throw vs 서버 401** — write 호출에 키 없으면 *서버에
  보내기 전* throw/raise. 서버 401만 받으면 *어디가 잘못됐는지 불분명*
  (Cookbook 예시: 운영자가 `client.createContractLabel(...)` 호출했는데
  401 → "키가 잘못됐나? URL이 잘못됐나? 라벨 형식이 잘못됐나?"). 사전 throw는
  *call site*에 즉시 신호 → 디버깅 마찰 ↓. D021 info-leak 방지와 *상호 보완*
  (서버는 정보 X, 클라이언트가 자기가 보낸 게 뭔지 알아서 더 친절).
- **info-leak 방지의 *사용 측* 영향 (D021)** — 서버가 401 단일 응답 → 운영자가
  *왜* 401인지 디버깅 어려움. cookbook의 "If you forget the API key" 절이
  이 trade-off를 *명시적으로 가르침*: 모든 401은 *같은 메시지*, 그래서 운영자
  체크리스트(키 박혔는지 / Bearer prefix 맞는지 / 정확한 키인지)가 cookbook
  쪽으로 이전.
- **shell 스크립트 환경 변수 강제 패턴 (`:?`)** — `: "${VAR:?required (S16/M006)}"`
  은 *bash 자체*가 missing variable로 인식해 즉시 stderr + 비-0 exit. `if [ -z "$VAR" ]`
  보다 정직 + 단순. 운영자가 잊고 실행해도 *script 첫 줄에서* 실패 → 환경 디버깅
  비용 ↓.
- **verify 스크립트의 *cleanup curl*에도 인증 헤더 필요** — `trap cleanup EXIT`
  로 등록된 cleanup이 보호 라우트(DELETE) 호출하면 키 없이 *조용히 401* → 리소스
  누수. 모든 보호 라우트 호출 자리(*cleanup 포함*)에 헤더 부착 필요.

## 정직한 한계 / 잔여

- **프론트 `/alerts` 페이지 깨짐** — write 버튼이 401. **S18에서 키 입력 UI
  도입**으로 해소가 명시적 의존(GSD-2 원칙: 출하 전 분해 금지 일관 → S18 sketch
  해제는 본 슬라이스 출하 시점).
- **examples는 컴파일/타입 검증까지** — 라이브 401 시뮬은 verify 스크립트가
  HTTP 레벨에서 검증, examples는 *호출 방법 시연* 책임 (D017 정신).
- **라이브 메인넷 자동 회귀 부재** — 모든 검증은 docker compose 시드 데이터.
  메인넷 트래픽 자동 회귀는 환경 부재로 불가능 (M004/M005 일관).
- **회전 = env 갱신 + 재시작** (D021/D023 일관) — 본 슬라이스에서 절차만 docs에
  명시. 무중단 회전은 multi-key runtime, *별 슬라이스*(M006 출하 이후 후속).
- **rate limiting / audit log 미부착** — write 라우트만 보호하므로 brute-force
  표면은 좁고, 401 응답이 attacker에게 키 정보 X (S16/D021). 별 단위.
- **API_PORT 기본 3001 충돌** — 검증 환경에서 외부 프로세스가 3001 점유.
  검증 명령에 `API_PORT=3005` 명시. 본 슬라이스 출하 시점 운영 영향 없음
  (verify 스크립트는 환경 변수로 조정 가능, 기존 패턴 일관).

## Reassess

ROADMAP **M006 S17 `[x] DONE`**. **S18 sketch 해제** + 태스크 분해 박음
(키 입력 UI / 401 처리 / 키 미설정 시 write 버튼 비활성 / M006 마감 cookbook).
S18 출하 = **M006 마감 슬라이스** (M006 ✅ SHIPPED 선언).

다음 호흡: **S18 진입** — 프론트 `/alerts` 페이지에 *세션 메모리* 기반 키 입력
UI + 401 응답 처리 + 키 미설정 시 write 버튼 비활성. M006 마감 시점에 *세
페르소나*(dApp 개발자 / 봇 운영자 / 운영자) 모두 *안전*하게 사용 가능한 상태.
