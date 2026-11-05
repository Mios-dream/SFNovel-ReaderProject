/**
 * 作品信息的共用格式化。
 *
 * 这里只放「同一事实只有一种展示规则」的函数。语义不同的同名函数不要合并：
 * 首页的 `formatMetric` 在无数据时返回空串、评论页的取整规则也不同，它们各自
 * 保留在所在页面。
 */

/** 数值达到 100 后不再保留小数，否则保留一位并去掉末尾的 `.0`。 */
export function trimMetric(value: number): string {
  return value >= 100 ? value.toFixed(0) : value.toFixed(1).replace(/\.0$/, "");
}

/** 把阅读量等大数字缩写为「亿 / 万」，非法值返回 `--`。 */
export function formatMetric(value?: number): string {
  if (typeof value !== "number" || !Number.isFinite(value)) return "--";
  const absolute = Math.abs(value);
  if (absolute >= 100_000_000) {
    return `${trimMetric(value / 100_000_000)}亿`;
  }
  if (absolute >= 10_000) {
    return `${trimMetric(value / 10_000)}万`;
  }
  return Math.round(value).toLocaleString("zh-CN");
}

/** 评分对应的色调类名，用于详情页分数徽标。 */
export function scoreToneClass(value?: number): string {
  if (typeof value !== "number") return "score-unknown";
  if (value >= 9) return "score-excellent";
  if (value >= 8) return "score-good";
  if (value >= 6) return "score-average";
  return "score-low";
}

/** 把上游日期转成本地化日期；无法解析时原样返回，缺失时返回 `--`。 */
export function formatSourceDate(value?: string): string {
  if (!value) return "--";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleDateString("zh-CN");
}

/** 列表里的「几月几日」，缺失或无法解析时统一显示「未知时间」。 */
export function formatMonthDay(value?: string): string {
  if (!value) return "未知时间";
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? "未知时间"
    : new Intl.DateTimeFormat("zh-CN", {
        month: "short",
        day: "numeric",
      }).format(date);
}
