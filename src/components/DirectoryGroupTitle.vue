<script lang="ts">
/**
 * 分组标题的行高。
 *
 * 目录列表是虚拟列表，行高由各页构建行时自己记账；这里导出让记账与渲染共用一个数，
 * 否则会出现"声明 42px、渲染 44px"这类每卷累计一段滚动偏移的问题。
 */
export const DIRECTORY_GROUP_HEIGHT = 44;
</script>

<script setup lang="ts">
/**
 * 目录列表里的分组标题：小说/有声/漫画的卷名，或"全部章节"这类分区名。
 *
 * 只服务目录，样式属于目录本身，不放进全局样式表——它和 `.drawer-state` 那种
 * 所有抽屉共用的排版不是一回事。层级固定为 `h3`：抽屉标题是 `h2`，卷名低于它。
 */
defineProps<{ label: string }>();
</script>

<template>
  <h3 class="directory-group-title">{{ label }}</h3>
</template>

<style scoped>
/* height 必须与 DIRECTORY_GROUP_HEIGHT 一致（虚拟列表按它计算偏移）。 */
.directory-group-title {
  display: flex;
  height: 44px;
  align-items: center;
  margin: 0;
  padding: 0 20px;
  overflow: hidden;
  color: #887f70;
  font-size: 12px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
