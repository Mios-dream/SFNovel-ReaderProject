import type { InjectionKey } from "vue";
import type { AuthDesk } from "./domains/auth";
import type { BookshelfDesk } from "./domains/bookshelf";
import type { JobsDesk } from "./domains/jobs";
import type { LibraryDesk } from "./domains/library";
import type { OnlineDesk } from "./domains/online";
import type { PickerDesk } from "./domains/picker";
import type { SearchDesk } from "./domains/search";
import type { SettingsDesk } from "./domains/settings";
import type { SigninDesk } from "./domains/signin";
import type { UpdaterDesk } from "./domains/updater";
import type { RemoteDesk } from "./domains/remote";
import type { WorkspaceDesk } from "./domains/workspace";

/**
 * 每个域一个注入键。
 *
 * 键按域切分而不是共用一个 `desk` 对象：页面只能拿到自己 import 的那个域，域与域
 * 之间没有可达路径，键名冲突（例如 `chapterLoading`）也就无法再互相遮蔽。
 */
function domainKey<T>(name: string): InjectionKey<T> {
  return Symbol(`desk.${name}`);
}

export const authKey = domainKey<AuthDesk>("auth");
export const libraryKey = domainKey<LibraryDesk>("library");
export const jobsKey = domainKey<JobsDesk>("jobs");
export const bookshelfKey = domainKey<BookshelfDesk>("bookshelf");
export const searchKey = domainKey<SearchDesk>("search");
export const settingsKey = domainKey<SettingsDesk>("settings");
export const signinKey = domainKey<SigninDesk>("signin");
export const updaterKey = domainKey<UpdaterDesk>("updater");
export const onlineKey = domainKey<OnlineDesk>("online");
export const pickerKey = domainKey<PickerDesk>("picker");
export const remoteKey = domainKey<RemoteDesk>("remote");
export const workspaceKey = domainKey<WorkspaceDesk>("workspace");
