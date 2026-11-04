import { computed, onBeforeUnmount, ref, watch } from "vue";

export type VirtualRow = {
  key: string | number;
  height: number;
};

export function useVirtualRows<Row extends VirtualRow>(
  readRows: () => Row[],
  overscan = 6,
) {
  const container = ref<HTMLElement>();
  const scrollTop = ref(0);
  const viewportHeight = ref(1200);
  const resizeObserver = new ResizeObserver(() => {
    if (container.value) viewportHeight.value = container.value.clientHeight;
  });
  watch(
    container,
    (element) => {
      resizeObserver.disconnect();
      if (!element) return;
      resizeObserver.observe(element);
      viewportHeight.value = element.clientHeight;
    },
    { flush: "post" },
  );
  onBeforeUnmount(() => resizeObserver.disconnect());

  const offsets = computed(() => {
    const result: number[] = [];
    let offset = 0;
    for (const row of readRows()) {
      result.push(offset);
      offset += row.height;
    }
    result.push(offset);
    return result;
  });

  const windowed = computed(() => {
    const rows = readRows();
    const starts = offsets.value;
    const top = scrollTop.value;
    let start = 0;
    while (start < rows.length - 1 && starts[start + 1] <= top) start++;
    let end = start;
    while (end < rows.length && starts[end] < top + viewportHeight.value) end++;
    const from = Math.max(0, start - overscan);
    const to = Math.min(rows.length, end + overscan);
    return {
      rows: rows.slice(from, to),
      top: starts[from],
      bottom: starts[rows.length] - starts[to],
    };
  });

  function handleScroll() {
    const element = container.value;
    if (!element) return;
    scrollTop.value = element.scrollTop;
    viewportHeight.value = element.clientHeight;
  }

  function reset() {
    scrollTop.value = 0;
    if (container.value) container.value.scrollTop = 0;
  }

  return { container, windowed, handleScroll, reset };
}
