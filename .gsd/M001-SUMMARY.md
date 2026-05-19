---
milestone: M001
title: Failure Intelligence Core API
status: SHIPPED
slices: [S01, S02, S03, S04]   # + 백로그 TEST-HARNESS
gate: pass
ship_definition: "인덱싱된 임의의 실패 tx를 조회·진단할 수 있고, 외부 제품이 임베드할 수 있다."
---

# M001 — SHIPPED

## 수용 기준 (REQUIREMENTS.md#M001) — 항목별 검증

| 기준 | 상태 | 증빙 |
|------|------|------|
| `GET /v1/failed-tx/{tx_hash}` → revert 사유+카테고리+평탄 콜트리, 미존재 404 | ✅ | verify: GOOD 200 + `ORDER OK`, BAD 404 (+ malformed 400, 하드닝) |
| `GET /v1/failed-tx?category&from&to&limit&offset` → 필터·페이지 + 정확 total | ✅ | verify: `total=3, returned=2` (limit 독립) |
| `GET /v1/analytics/failed-tx/timeseries?interval&from&to` → 카테고리 추이 | ✅ | verify: day 200, bogus 400 |
| 3개가 `ApiResponse`/`PaginatedResponse` 계약·에러 규약 준수 | ✅ | `ApiResponse`/`TotalPaginatedResponse`(가산적, D005); `{error}` 400/404 |
| curl 재현 스크립트 + 엔드포인트 문서 존재 (임베드 증빙) | ✅ | `scripts/verify-failed-tx.sh`, `docs/api-failed-tx.md`, README 링크 |
| 비기능: no prod unwrap / 파라미터화 SQL / `///` / 통합검증 | ✅ | `?`·`ok_or_else`(expect는 테스트만), 바인딩·인젝션안전 date_trunc, db 통합테스트 8 + HTTP 스크립트 |

전체 게이트(최종 재실행): clippy 0 (db+api+tests) · fmt OK · `cargo test -p db --ignored` 8/8 ·
`verify-failed-tx.sh` ALL PASS.

## 무엇이 만들어졌나 (제품 관점)

기존 `trace.rs`+`classifier.rs` 자산 위에, **개별 진단 → 목록 → 추세**의 실패
인텔리전스 3종 API. Dune이 out-of-box로 못 주는 trace 레벨 질문에 임베드형 계약으로
응답. 일반 대시보드(Dune 강점)는 의도적으로 미투자(D001).

## 핵심 교훈 (KNOWLEDGE 반영)

- 검증은 shape가 아니라 **semantics**(H1 정렬 버그가 스모크 통과 → 불변식 테스트로 박음)
- 검증 경계: 잘못된 입력 **400**, 없음 **404** — 일관 적용
- 가산적 계약 진화(`TotalPaginatedResponse`), str→enum 단일 출처(`FromStr`)
- 인젝션 안전 동적 집계(enum 화이트리스트 + 바인딩 + ordinal)
- 마일스톤 검증 = **누적 green 불신, 게이트 전체 재실행**(작업 중 Docker 다운으로
  통합테스트 전멸 → 환경/코드 분리 진단 후 복구)

## 다음 (Reassess)

M001 출하로 M002 분해 금지 해제. **M002 — Real-time Failure Pipeline**
(S05 체인 헤드 팔로워 / S06 reorg 정정 — `[edge: untapped]`, risk: high, 학습 핵심)
가 다음 후보. 분해는 다음 지시 시 S05-PLAN으로 진행(GSD-2: 다음 슬라이스만 분해).
