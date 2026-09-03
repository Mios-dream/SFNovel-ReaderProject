import type { InjectionKey } from "vue";
import type { useNovelDesk } from "./composables/useNovelDesk";

export type DeskContext = ReturnType<typeof useNovelDesk>;
export const deskInjectionKey: InjectionKey<DeskContext> = Symbol("desk");
