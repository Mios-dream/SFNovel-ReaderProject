import { inject, type InjectionKey } from "vue";
import {
  authKey,
  bookshelfKey,
  jobsKey,
  libraryKey,
  onlineKey,
  pickerKey,
  remoteKey,
  searchKey,
  settingsKey,
  signinKey,
  updaterKey,
  workspaceKey,
} from "./keys";
import type { AuthDesk } from "./domains/auth";
import type { BookshelfDesk } from "./domains/bookshelf";
import type { JobsDesk } from "./domains/jobs";
import type { LibraryDesk } from "./domains/library";
import type { OnlineDesk } from "./domains/online";
import type { PickerDesk } from "./domains/picker";
import type { SearchDesk } from "./domains/search";
import type { SettingsDesk } from "./domains/settings";
import type { SigninDesk } from "./domains/signin";
import type { RemoteDesk } from "./domains/remote";
import type { UpdaterDesk } from "./domains/updater";
import type { WorkspaceDesk } from "./domains/workspace";

export type { UpdateInfo, UpdateStatus } from "./domains/updater";

/**
 * 取某个域的上下文。
 *
 * 页面只在 `useDeskProvider` 之下渲染，缺失即为装配错误：直接抛出比返回
 * `undefined` 更容易定位——后者要到第一个属性访问处才炸，栈里看不出根因。
 */
function requireDomain<T>(key: InjectionKey<T>, name: string): T {
  const context = inject(key);
  if (!context) throw new Error(`Desk context "${name}" is unavailable`);
  return context;
}

export function useAuth(): AuthDesk {
  return requireDomain(authKey, "auth");
}
export function useLibrary(): LibraryDesk {
  return requireDomain(libraryKey, "library");
}
export function useJobs(): JobsDesk {
  return requireDomain(jobsKey, "jobs");
}
export function useBookshelf(): BookshelfDesk {
  return requireDomain(bookshelfKey, "bookshelf");
}
export function useSearch(): SearchDesk {
  return requireDomain(searchKey, "search");
}
export function useSettings(): SettingsDesk {
  return requireDomain(settingsKey, "settings");
}
export function useSignin(): SigninDesk {
  return requireDomain(signinKey, "signin");
}
export function useUpdater(): UpdaterDesk {
  return requireDomain(updaterKey, "updater");
}
export function useOnline(): OnlineDesk {
  return requireDomain(onlineKey, "online");
}
export function usePicker(): PickerDesk {
  return requireDomain(pickerKey, "picker");
}
export function useRemote(): RemoteDesk {
  return requireDomain(remoteKey, "remote");
}
export function useWorkspace(): WorkspaceDesk {
  return requireDomain(workspaceKey, "workspace");
}
