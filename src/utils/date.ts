/** 一个不含时区的日历日期。 */
export type CalendarDate = { year: number; month: number; day: number };

/**
 * 北京时间的当前日历日。
 *
 * 签到与上游都以 UTC+8 计算「今天」，因此不能用设备本地时区：设备时区偏西时
 * 本地日期会比上游晚一天。`Intl` 负责时区换算，避免手工加减偏移量。
 */
export function beijingToday(): CalendarDate {
  const parts = new Intl.DateTimeFormat("en-US", {
    timeZone: "Asia/Shanghai",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).formatToParts(new Date());
  const value = (type: string) =>
    Number(parts.find((part) => part.type === type)?.value || 0);
  return { year: value("year"), month: value("month"), day: value("day") };
}

/** `YYYY-MM-DD`；与上游 `date` 字段的前 10 位同构。 */
export function calendarDateKey(date: CalendarDate) {
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.year}-${pad(date.month)}-${pad(date.day)}`;
}

/** 上游日期可能是带时间的完整时间戳，只比较日期部分。 */
export function isSameCalendarDay(value: string, date: CalendarDate) {
  return value.slice(0, 10) === calendarDateKey(date);
}
