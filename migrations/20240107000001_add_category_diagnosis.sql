-- S12 (M004): category_diagnosis 자기소유 시드.
-- error_category (SCREAMING_SNAKE wire form) → 사람이 읽는 진단 메시지 + 추천 액션.
-- /v1/failed-tx/{tx_hash} 응답에서 dApp 개발자가 "왜 실패했나 + 어떻게 고치나"를
-- 한 호출에 받게 한다. enum 자체의 세분화는 별 슬라이스 (S12.1 sketch, D016).
-- 외부 의존 미도입 — 자기 시드만 (D008 / D015 정신).

BEGIN;

CREATE TABLE IF NOT EXISTS category_diagnosis (
    error_category     TEXT PRIMARY KEY,
    message            TEXT NOT NULL,
    recommended_action TEXT,
    source             TEXT,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE category_diagnosis IS
  'error_category (SCREAMING_SNAKE wire form) → 사람이 읽는 진단 메시지 + 추천 액션 (S12 / M004). 자기 시드만.';
COMMENT ON COLUMN category_diagnosis.error_category IS
  'ErrorCategory enum wire form (UNKNOWN, INSUFFICIENT_BALANCE, SLIPPAGE_EXCEEDED, ...)';
COMMENT ON COLUMN category_diagnosis.message IS
  '사람이 읽는 진단 메시지 1줄 — 왜 실패했나';
COMMENT ON COLUMN category_diagnosis.recommended_action IS
  '권장 액션 1줄 — 어떻게 고치나 (선택)';
COMMENT ON COLUMN category_diagnosis.source IS
  '시드 출처 (예: builtin) — 운영자가 후속 시드 시 구분';

-- 6 카테고리 1행씩 시드. ON CONFLICT DO NOTHING으로 멱등 (재실행 안전).
INSERT INTO category_diagnosis (error_category, message, recommended_action, source) VALUES
  ('UNKNOWN',
   'The exact failure mode could not be classified from the trace alone.',
   'Inspect root_cause and the call_tree; raise an issue with the tx hash.',
   'builtin'),
  ('INSUFFICIENT_BALANCE',
   'Sender lacks the token or ETH balance needed for the operation.',
   'Verify the sender holds enough of the input token (or wrap ETH first).',
   'builtin'),
  ('SLIPPAGE_EXCEEDED',
   'The trade output was below the minimum acceptable amount (price slippage).',
   'Increase slippage tolerance, or split the trade to reduce price impact.',
   'builtin'),
  ('DEADLINE_EXPIRED',
   'The transaction was mined after its specified deadline.',
   'Resubmit with a later deadline (or a tighter gas-price target).',
   'builtin'),
  ('UNAUTHORIZED',
   'The caller lacks permission for this operation (ownership or approval).',
   'Approve the spender first, or confirm the caller is the owner.',
   'builtin'),
  ('TRANSFER_FAILED',
   'An ERC-20 transfer reverted (returned false or threw).',
   'Check token balance, allowance, and whether the token has transfer hooks.',
   'builtin')
ON CONFLICT (error_category) DO NOTHING;

COMMIT;
