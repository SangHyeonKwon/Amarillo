/** Shared Recharts styling so every chart matches the dark theme. */

export const chartBrand = "#3ECF8E";
export const chartWarning = "#F4BD50";
export const chartMuted = "#888888";

export const axisTick = { fontSize: 12, fill: "#888888" };
export const axisLine = { stroke: "#333333" };
export const gridStroke = "#333333";

export const tooltipContentStyle = {
  background: "#2E2E2E",
  border: "1px solid #333333",
  borderRadius: 6,
  fontSize: 13,
  padding: "8px 10px",
};
export const tooltipItemStyle = { color: "#EDEDED" };
export const tooltipLabelStyle = { color: "#888888", marginBottom: 4 };

/** Recharts `ValueType` is loose; our series values are always numeric. */
export function numeric(value: unknown): number {
  return typeof value === "number" ? value : Number(value) || 0;
}
