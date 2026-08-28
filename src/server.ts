import express from "express";
import axios from "axios";
import fse from "fs-extra";
import path from "node:path";
import { createWriteStream } from "node:fs";
import { spawn } from "node:child_process";
import { pipeline } from "node:stream/promises";
import crypto from "node:crypto";
import WebSocket from "ws";
import { SfacgClient } from "./client/Sfacg/api/client";
import type { IsearchInfos, IvolumeInfos } from "./client/Sfacg/types/ITypes";

const app = express();
const rootDir = process.cwd();
const libraryDir = path.join(rootDir, "output", "菠萝包轻小说");
const webDir = path.join(rootDir, "dist");

type Job = {
  id: string;
  title: string;
  status: "queued" | "downloading" | "paused" | "done" | "error" | "cancelled";
  progress: number;
  message: string;
  file?: string;
  kind: "text" | "audio";
  novelId: number;
  chapterIds?: number[];
};
type AuthSession = { cookie: string; userName: string };
type DevToolsTab = { type: string; url: string; webSocketDebuggerUrl?: string };
type DevToolsCookie = { name: string; value: string; domain: string };
type DevToolsVersion = { webSocketDebuggerUrl?: string };
type ControlledBrowserSession = { cookie: string; loginCompleted: boolean };
type AudioChapter = {
  id: number;
  title: string;
  source: string;
  volume: string;
};
type NovelDownloadMetadata = {
  novelId?: number;
  title?: string;
  downloadedTextChapterIds?: number[];
  downloadedAudioChapterIds?: number[];
};
type AudioInfoResponse = {
  status?: number;
  msg?: string;
  data?: {
    NovelName?: string;
    VolumeSet?: Array<{
      VolumeName?: string;
      AudioSet?: Array<{
        AudioID?: number;
        ChapterTitle?: string;
        AudioSrc?: string;
      }>;
    }>;
  };
};
type AudioCatalogErrorCode = "NO_AUDIO" | "AUTH_EXPIRED" | "UPSTREAM_ERROR";
class AudioCatalogError extends Error {
  constructor(
    public readonly code: AudioCatalogErrorCode,
    message: string,
    public readonly httpStatus: 401 | 404 | 502,
  ) {
    super(message);
    this.name = "AudioCatalogError";
  }
}
const jobs = new Map<string, Job>();
const jobControllers = new Map<string, AbortController>();
const jobSessions = new Map<
  string,
  { cookie?: string; chapterIds?: number[] }
>();
const localAuthCookie = "sfacg_session";
const localAuthMaxAge = 30 * 24 * 60 * 60 * 1000;
const browserDebugPort = 9223;
const browserProfileDir = path.join(rootDir, ".sfacg-login-profile");
const browserLoginUrl = "https://passport.sfacg.com/Login.aspx";
let browserLaunchPendingUntil = 0;

type CacheEntry<T> = { expiresAt: number; value: T };
const responseCache = new Map<string, CacheEntry<unknown>>();
const pendingCacheLoads = new Map<string, Promise<unknown>>();
const metadataCacheTtl = 60_000;
const audioCacheTtl = 120_000;
const bookshelfCacheTtl = 120_000;
let metadataQueue: Promise<void> = Promise.resolve();
let lastMetadataRequestAt = 0;

function sessionKey(cookie: string | undefined, scope: string) {
  const identity = cookie
    ? crypto.createHash("sha256").update(cookie).digest("hex")
    : "anonymous";
  return `${scope}:${identity}`;
}

async function cached<T>(
  key: string,
  ttl: number,
  loader: () => Promise<T>,
): Promise<T> {
  const existing = responseCache.get(key) as CacheEntry<T> | undefined;
  if (existing && existing.expiresAt > Date.now()) return existing.value;
  const pending = pendingCacheLoads.get(key) as Promise<T> | undefined;
  if (pending) return pending;
  const load = loader()
    .then((value) => {
      responseCache.set(key, { value, expiresAt: Date.now() + ttl });
      return value;
    })
    .finally(() => pendingCacheLoads.delete(key));
  pendingCacheLoads.set(key, load);
  return load;
}

// Serialize metadata probes and leave a small gap between upstream requests.
async function throttledMetadata<T>(
  loader: () => Promise<T>,
  interval = 250,
): Promise<T> {
  const run = metadataQueue.then(async () => {
    const wait = Math.max(0, interval - (Date.now() - lastMetadataRequestAt));
    if (wait) await new Promise((resolve) => setTimeout(resolve, wait));
    try {
      return await loader();
    } finally {
      lastMetadataRequestAt = Date.now();
    }
  });
  metadataQueue = run.then(
    () => undefined,
    () => undefined,
  );
  return run;
}

app.use(express.json());
app.use("/library", express.static(libraryDir));

const safeName = (name: string) => name.replace(/[<>:"/\\|?*]/g, "_").trim();
const safeAudioName = (name: string) =>
  safeName(name).replace(/[. ]+$/g, "") || "未命名章节";

function bookUrl(folder: string, file: string) {
  return `/library/${encodeURIComponent(folder)}/${encodeURIComponent(file)}`;
}

function numberIds(value: unknown) {
  return Array.isArray(value)
    ? value.filter((id): id is number => Number.isInteger(id) && id > 0)
    : [];
}

async function readNovelDownloadMetadata(
  dir: string,
): Promise<NovelDownloadMetadata> {
  try {
    const metadata = (await fse.readJson(
      path.join(dir, ".novel-flow.json"),
    )) as NovelDownloadMetadata;
    return metadata && typeof metadata === "object" ? metadata : {};
  } catch {
    return {};
  }
}

async function writeNovelDownloadMetadata(
  dir: string,
  metadata: NovelDownloadMetadata,
) {
  await fse.outputFile(
    path.join(dir, ".novel-flow.json"),
    JSON.stringify({
      ...metadata,
      downloadedTextChapterIds: numberIds(metadata.downloadedTextChapterIds),
      downloadedAudioChapterIds: numberIds(metadata.downloadedAudioChapterIds),
    }),
  );
}

async function getLocalDownloadState(novelId: number) {
  await fse.ensureDir(libraryDir);
  const folders = await fse.readdir(libraryDir, { withFileTypes: true });
  const states = await Promise.all(
    folders
      .filter((folder) => folder.isDirectory())
      .map(async (folder) => {
        const dir = path.join(libraryDir, folder.name);
        const metadata = await readNovelDownloadMetadata(dir);
        if (
          metadata.novelId !== novelId ||
          numberIds(metadata.downloadedTextChapterIds).length
        )
          return { metadata, downloadedTextTitles: [] as string[] };
        // Downloads created before chapter IDs were recorded can still be shown as
        // completed by reading the Markdown headings once during the migration.
        const files = await fse.readdir(dir);
        const markdown = files.find((file) => file.endsWith(".md"));
        if (!markdown)
          return { metadata, downloadedTextTitles: [] as string[] };
        try {
          const content = await fse.readFile(path.join(dir, markdown), "utf8");
          return {
            metadata,
            downloadedTextTitles: [...content.matchAll(/^##\s+(.+)$/gm)].map(
              (match) => match[1].trim(),
            ),
          };
        } catch {
          return { metadata, downloadedTextTitles: [] as string[] };
        }
      }),
  );
  const matching = states.filter(
    ({ metadata }) => metadata.novelId === novelId,
  );
  return {
    downloadedTextChapterIds: [
      ...new Set(
        matching.flatMap(({ metadata }) =>
          numberIds(metadata.downloadedTextChapterIds),
        ),
      ),
    ],
    downloadedTextTitles: [
      ...new Set(
        matching.flatMap(({ downloadedTextTitles }) => downloadedTextTitles),
      ),
    ],
    downloadedAudioChapterIds: [
      ...new Set(
        matching.flatMap(({ metadata }) =>
          numberIds(metadata.downloadedAudioChapterIds),
        ),
      ),
    ],
  };
}

function getAuthSession(req: express.Request): AuthSession | undefined {
  const token = req.headers.cookie
    ?.split(/;\s*/)
    .find((item) => item.startsWith(`${localAuthCookie}=`))
    ?.slice(localAuthCookie.length + 1);
  if (!token) return undefined;
  try {
    const parsed = JSON.parse(
      Buffer.from(token, "base64url").toString("utf8"),
    ) as Partial<AuthSession>;
    return typeof parsed.cookie === "string" &&
      typeof parsed.userName === "string"
      ? { cookie: parsed.cookie, userName: parsed.userName }
      : undefined;
  } catch {
    return undefined;
  }
}

function saveAuthSession(res: express.Response, session: AuthSession) {
  res.cookie(
    localAuthCookie,
    Buffer.from(JSON.stringify(session)).toString("base64url"),
    {
      httpOnly: true,
      sameSite: "strict",
      maxAge: localAuthMaxAge,
      path: "/",
    },
  );
}

function findBrowserExecutable() {
  const programFiles = process.env.ProgramFiles || "C:\\Program Files";
  const programFilesX86 =
    process.env["ProgramFiles(x86)"] || "C:\\Program Files (x86)";
  const candidates = [
    path.join(programFiles, "Microsoft", "Edge", "Application", "msedge.exe"),
    path.join(
      programFilesX86,
      "Microsoft",
      "Edge",
      "Application",
      "msedge.exe",
    ),
    path.join(programFiles, "Google", "Chrome", "Application", "chrome.exe"),
    path.join(programFilesX86, "Google", "Chrome", "Application", "chrome.exe"),
  ];
  return candidates.find((candidate) => fse.pathExistsSync(candidate));
}

function hasCompletedOfficialLogin(url: string) {
  try {
    const pageUrl = new URL(url);
    const host = pageUrl.hostname.toLowerCase();
    if (!host.endsWith(".sfacg.com") && host !== "sfacg.com") return false;
    if (host !== "passport.sfacg.com") return true;
    return (
      pageUrl.pathname.toLowerCase().endsWith("/message.aspx") &&
      pageUrl.searchParams.get("msg") === "LoginSuccessToHome"
    );
  } catch {
    return false;
  }
}

async function getControlledBrowserSession(): Promise<
  ControlledBrowserSession | undefined
> {
  try {
    const { data } = await axios.get<DevToolsTab[]>(
      `http://127.0.0.1:${browserDebugPort}/json/list`,
      { timeout: 700 },
    );
    const sfPages = data.filter(
      (tab) =>
        tab.type === "page" &&
        tab.url.includes("sfacg.com") &&
        tab.webSocketDebuggerUrl,
    );
    const page =
      sfPages.find((tab) => hasCompletedOfficialLogin(tab.url)) || sfPages[0];
    if (!page?.webSocketDebuggerUrl) return undefined;
    const cookies = await new Promise<DevToolsCookie[]>((resolve, reject) => {
      const socket = new WebSocket(page.webSocketDebuggerUrl!);
      const timeout = setTimeout(() => {
        socket.terminate();
        reject(new Error("读取官方登录会话超时"));
      }, 2500);
      socket.once("open", () =>
        socket.send(JSON.stringify({ id: 1, method: "Network.getAllCookies" })),
      );
      socket.on("message", (raw) => {
        try {
          const message = JSON.parse(raw.toString()) as {
            id?: number;
            result?: { cookies?: DevToolsCookie[] };
          };
          if (message.id === 1) {
            clearTimeout(timeout);
            socket.close();
            resolve(message.result?.cookies || []);
          }
        } catch (error) {
          clearTimeout(timeout);
          socket.terminate();
          reject(error);
        }
      });
      socket.once("error", (error) => {
        clearTimeout(timeout);
        reject(error);
      });
    });
    // Only forward cookies that the API host can receive. A browser profile can
    // contain same-named host-only cookies for passport/i.sfacg.com.
    const sfCookies = cookies.filter(
      (cookie) =>
        (cookie.domain === "sfacg.com" ||
          cookie.domain === ".sfacg.com" ||
          cookie.domain === "api.sfacg.com") &&
        (cookie.name === ".SFCommunity" ||
          cookie.name.startsWith("session_") ||
          cookie.name.startsWith(".SF")),
    );
    if (!sfCookies.some((cookie) => cookie.name === ".SFCommunity"))
      return undefined;
    const cookie = [
      ...new Map(
        sfCookies
          .sort(
            (left, right) =>
              Number(right.domain === "api.sfacg.com") -
              Number(left.domain === "api.sfacg.com"),
          )
          .map((cookie) => [cookie.name, cookie.value]),
      ).entries(),
    ]
      .map(([name, value]) => `${name}=${value}`)
      .join("; ");
    return { cookie, loginCompleted: hasCompletedOfficialLogin(page.url) };
  } catch {
    return undefined;
  }
}

async function isControlledBrowserRunning() {
  try {
    await axios.get(`http://127.0.0.1:${browserDebugPort}/json/version`, {
      timeout: 500,
    });
    return true;
  } catch {
    return false;
  }
}

async function closeControlledBrowser() {
  try {
    const { data } = await axios.get<DevToolsVersion>(
      `http://127.0.0.1:${browserDebugPort}/json/version`,
      { timeout: 700 },
    );
    if (!data.webSocketDebuggerUrl) return;
    await new Promise<void>((resolve) => {
      const socket = new WebSocket(data.webSocketDebuggerUrl!);
      const timeout = setTimeout(() => {
        socket.terminate();
        resolve();
      }, 1500);
      socket.once("open", () =>
        socket.send(JSON.stringify({ id: 1, method: "Browser.close" })),
      );
      socket.once("message", () => {
        clearTimeout(timeout);
        socket.close();
        resolve();
      });
      socket.once("error", () => {
        clearTimeout(timeout);
        resolve();
      });
    });
  } catch {
    // The browser may already have been closed by the user.
  }
}

async function writeNovel(
  novelId: number,
  cookie: string | undefined,
  chapterIds: number[] | undefined,
  signal: AbortSignal,
  onProgress?: (value: number, message: string) => void,
) {
  const client = new SfacgClient();

  const [novel, volumes] = await Promise.all([
    client.novelInfo(novelId, signal),
    client.volumeInfos(novelId, signal),
  ]);
  if (!novel || !volumes) throw new Error("无法读取小说信息");

  const novelName = safeName(novel.novelName);
  const novelDir = path.join(libraryDir, novelName);
  const imageDir = path.join(novelDir, "imgs");
  await fse.ensureDir(imageDir);
  const selected = chapterIds?.length ? new Set(chapterIds) : undefined;
  const total = volumes.reduce(
    (sum, volume) =>
      sum +
      volume.chapterList.filter(
        (chapter) => !selected || selected.has(chapter.chapId),
      ).length,
    0,
  );
  let finished = 0;
  const downloadClient = new SfacgClient();
  if (cookie) downloadClient.SetCookie(cookie);
  const progressFile = path.join(novelDir, ".novel-flow-progress.json");
  let savedChapters: Record<
    string,
    { volume: string; title: string; content: string }
  > = {};
  try {
    const saved = await fse.readJson(progressFile);
    if (
      saved?.novelId === novelId &&
      saved.chapters &&
      typeof saved.chapters === "object"
    )
      savedChapters = saved.chapters;
  } catch {
    /* a fresh download has no progress file */
  }
  const metadata = await readNovelDownloadMetadata(novelDir);
  const downloadedTextChapterIds = new Set(
    numberIds(metadata.downloadedTextChapterIds),
  );

  onProgress?.(4, "正在准备书籍文件");
  const content: string[] = [];
  for (const volume of volumes) {
    const chapters: string[] = [];
    for (const chapter of volume.chapterList) {
      if (selected && !selected.has(chapter.chapId)) continue;
      if (signal.aborted) throw new Error("下载已取消");
      const saved = savedChapters[String(chapter.chapId)];
      if (saved) {
        finished += 1;
        chapters.push(saved.content);
        continue;
      }
      try {
        if (chapter.needFireMoney !== 0 && !cookie) continue;
        const raw = await downloadClient.contentInfos(chapter.chapId, signal);
        if (signal.aborted) throw new Error("下载已取消");
        finished += 1;
        onProgress?.(
          total ? Math.round(5 + (finished / total) * 90) : 100,
          `正在下载：${chapter.ntitle}`,
        );
        if (raw) {
          const content = `## ${chapter.ntitle}\n\n${raw.replaceAll("\n", "\n\n")}`;
          savedChapters[String(chapter.chapId)] = {
            volume: volume.title,
            title: chapter.ntitle,
            content,
          };
          downloadedTextChapterIds.add(chapter.chapId);
          chapters.push(content);
          await fse.outputFile(
            progressFile,
            JSON.stringify({ novelId, chapters: savedChapters }),
          );
          await writeNovelDownloadMetadata(novelDir, {
            ...metadata,
            novelId,
            title: novel.novelName,
            downloadedTextChapterIds: [...downloadedTextChapterIds],
          });
        }
      } catch (error) {
        if (signal.aborted) throw new Error("下载已取消");
        finished += 1;
      }
    }
    if (chapters.length)
      content.push(`# ${volume.title}\n\n${chapters.join("\n\n")}`);
  }

  const intro =
    novel.expand?.intro
      ?.split("\n")
      .map((line: string) => `  ${line}`)
      .join("\n") ?? "";
  const markdown = `---\ntitle: '${novel.novelName.replaceAll("'", "\\'")}'\nauthor: '${novel.authorName.replaceAll("'", "\\'")}'\nlang: 'zh-Hans'\ndescription: |-\n${intro}\n...\n\n${content.join("\n\n")}`;
  const markdownFile = `${novelName}.md`;
  await fse.outputFile(path.join(novelDir, markdownFile), markdown);
  await fse.remove(progressFile);
  await writeNovelDownloadMetadata(novelDir, {
    ...metadata,
    novelId,
    title: novel.novelName,
    downloadedTextChapterIds: [...downloadedTextChapterIds],
  });
  if (novel.novelCover) {
    try {
      await fse.outputFile(
        path.join(imageDir, "cover.jpeg"),
        await SfacgClient.image(novel.novelCover),
      );
    } catch {
      /* cover is optional */
    }
  }
  onProgress?.(100, "已保存到本地书库");
  return {
    name: novel.novelName,
    folder: novelName,
    file: markdownFile,
    chapters: finished,
  };
}

async function getAudioChapters(novelId: number, cookie: string) {
  let data: AudioInfoResponse;
  try {
    ({ data } = await axios.get<AudioInfoResponse>(
      "https://i.sfacg.com/ajax/ashx/Common.ashx",
      {
        params: { op: "getAudioInfo", nid: novelId },
        headers: {
          Cookie: cookie,
          "User-Agent":
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/131 Safari/537.36",
          Accept: "application/json, text/javascript, */*; q=0.01",
          "X-Requested-With": "XMLHttpRequest",
          Referer: "https://i.sfacg.com/consume/book/",
        },
        timeout: 15_000,
      },
    ));
  } catch (error) {
    if (
      axios.isAxiosError(error) &&
      (error.response?.status === 401 || error.response?.status === 403)
    ) {
      throw new AudioCatalogError(
        "AUTH_EXPIRED",
        "SF 登录会话已失效，请重新登录",
        401,
      );
    }
    const detail =
      axios.isAxiosError(error) && error.code === "ECONNABORTED"
        ? "SF 有声接口请求超时"
        : "无法连接 SF 有声接口";
    throw new AudioCatalogError("UPSTREAM_ERROR", detail, 502);
  }

  const upstreamStatus = Number(data.status);
  if (upstreamStatus === 401 || upstreamStatus === 403)
    throw new AudioCatalogError(
      "AUTH_EXPIRED",
      "SF 登录会话已失效，请重新登录",
      401,
    );
  if (upstreamStatus !== 200 || !data.data) {
    const upstreamMessage = data.msg || "";
    // SF uses status 400/"参数不正确" for novels without an audio
    // catalogue as well as for an unmatched audio record.
    if (upstreamStatus === 400 && upstreamMessage === "参数不正确") {
      throw new AudioCatalogError("NO_AUDIO", "该作品没有可用的有声章节", 404);
    }
    const detail = upstreamMessage ? `：${upstreamMessage}` : "";
    throw new AudioCatalogError(
      "UPSTREAM_ERROR",
      `SF 有声接口拒绝了请求${detail}`,
      502,
    );
  }
  const chapters: AudioChapter[] = [];
  for (const volume of data.data.VolumeSet || []) {
    for (const audio of volume.AudioSet || []) {
      if (audio.AudioSrc)
        chapters.push({
          id: Number(audio.AudioID || chapters.length + 1),
          title: audio.ChapterTitle || "未命名章节",
          source: audio.AudioSrc,
          volume: volume.VolumeName || "未分卷",
        });
    }
  }
  if (!chapters.length)
    throw new AudioCatalogError("NO_AUDIO", "该作品没有可用的有声章节", 404);
  return { title: data.data.NovelName || `小说 ${novelId}`, chapters };
}

async function writeAudio(
  novelId: number,
  cookie: string,
  chapterIds: number[] | undefined,
  signal: AbortSignal,
  onProgress?: (value: number, message: string) => void,
) {
  const audio = await getAudioChapters(novelId, cookie);
  const selected = chapterIds?.length ? new Set(chapterIds) : undefined;
  const chapters = selected
    ? audio.chapters.filter((chapter) => selected.has(chapter.id))
    : audio.chapters;
  const novelName = safeName(audio.title);
  const audioDir = path.join(libraryDir, novelName, "audio");
  await fse.ensureDir(audioDir);
  const playlist: string[] = ["#EXTM3U"];
  const metadata = await readNovelDownloadMetadata(
    path.join(libraryDir, novelName),
  );
  const downloadedAudioChapterIds = new Set(
    numberIds(metadata.downloadedAudioChapterIds),
  );

  for (const [index, chapter] of chapters.entries()) {
    if (signal.aborted) throw new Error("下载已取消");
    const filename = `${String(index + 1).padStart(3, "0")} - ${safeAudioName(chapter.title)}.mp3`;
    const target = path.join(audioDir, filename);
    const partialTarget = `${target}.part`;
    onProgress?.(
      Math.round((index / chapters.length) * 96) + 2,
      `正在下载：${chapter.title}`,
    );
    if (!(await fse.pathExists(target))) {
      try {
        const response = await axios.get<NodeJS.ReadableStream>(
          chapter.source,
          {
            responseType: "stream",
            signal,
            headers: { Cookie: cookie, "User-Agent": "Mozilla/5.0" },
            timeout: 60_000,
          },
        );
        await pipeline(response.data, createWriteStream(partialTarget));
        await fse.move(partialTarget, target, { overwrite: true });
      } catch (error) {
        await fse.remove(partialTarget);
        throw error;
      }
    }
    downloadedAudioChapterIds.add(chapter.id);
    await writeNovelDownloadMetadata(path.join(libraryDir, novelName), {
      ...metadata,
      novelId,
      title: audio.title,
      downloadedAudioChapterIds: [...downloadedAudioChapterIds],
    });
    playlist.push(`#EXTINF:-1,${chapter.volume} - ${chapter.title}`, filename);
  }
  const playlistFile = "有声目录.m3u8";
  await fse.outputFile(
    path.join(audioDir, playlistFile),
    `${playlist.join("\n")}\n`,
  );
  await writeNovelDownloadMetadata(path.join(libraryDir, novelName), {
    ...metadata,
    novelId,
    title: audio.title,
    downloadedAudioChapterIds: [...downloadedAudioChapterIds],
  });
  onProgress?.(100, "有声内容已保存到本地书库");
  return {
    name: audio.title,
    folder: novelName,
    file: path.posix.join("audio", playlistFile),
    chapters: chapters.length,
  };
}

app.get("/api/search", async (req, res) => {
  const query = String(req.query.q ?? "").trim();
  if (!query) return res.json([]);
  try {
    const results = await new SfacgClient().searchInfos(query);
    res.json(results || []);
  } catch (error) {
    res
      .status(500)
      .json({ message: error instanceof Error ? error.message : "搜索失败" });
  }
});

app.get("/api/bookshelf", async (req, res) => {
  const session = getAuthSession(req);
  if (!session)
    return res.status(401).json({ message: "请先登录 SF 账号后查看书架" });
  try {
    const client = new SfacgClient();
    client.SetCookie(session.cookie);
    const loadCollection = () => throttledMetadata(() => client.bookshelfCollection());
    const collection = req.query.refresh === "1"
      ? await loadCollection()
      : await cached(
          sessionKey(session.cookie, "bookshelf"),
          bookshelfCacheTtl,
          loadCollection,
        );
    if (!collection)
      return res
        .status(401)
        .json({ message: "SF 登录会话已失效，请重新登录后同步书架" });
    res.json(collection);
  } catch (error) {
    res.status(502).json({
      message:
        error instanceof Error
          ? `SF 书架请求失败：${error.message}`
          : "读取书架失败",
    });
  }
});

app.get("/api/chapters/:novelId", async (req, res) => {
  const novelId = Number(req.params.novelId);
  if (!Number.isInteger(novelId) || novelId <= 0)
    return res.status(400).json({ message: "小说编号无效" });
  try {
    const session = getAuthSession(req);
    const client = new SfacgClient();
    if (session) client.SetCookie(session.cookie);
    const volumes = await cached(
      sessionKey(session?.cookie, `chapters:${novelId}`),
      metadataCacheTtl,
      () => throttledMetadata(() => client.volumeInfos(novelId)),
    );
    const localState = await getLocalDownloadState(novelId);
    if (!volumes) throw new Error("无法读取章节目录");
    const downloaded = new Set(localState.downloadedTextChapterIds);
    const downloadedTitles = new Set(localState.downloadedTextTitles);
    res.json(
      volumes.map((volume) => ({
        volumeId: volume.volumeId,
        title: volume.title,
        chapters: volume.chapterList.map((chapter) => {
          const isDownloaded =
            downloaded.has(chapter.chapId) ||
            downloadedTitles.has(chapter.ntitle);
          return {
            chapId: chapter.chapId,
            title: chapter.ntitle,
            needFireMoney: chapter.needFireMoney,
            isVip: chapter.isVip,
            isUnlocked: Boolean(chapter.has) || isDownloaded,
            downloaded: isDownloaded,
          };
        }),
      })),
    );
  } catch (error) {
    res.status(500).json({
      message: error instanceof Error ? error.message : "读取章节目录失败",
    });
  }
});

app.get("/api/novel/:novelId", async (req, res) => {
  const novelId = Number(req.params.novelId);
  if (!Number.isInteger(novelId) || novelId <= 0)
    return res.status(400).json({ message: "小说编号无效" });
  try {
    const novel = await cached(`novel:${novelId}`, metadataCacheTtl, () =>
      throttledMetadata(() => new SfacgClient().novelInfo(novelId)),
    );
    if (!novel) throw new Error("无法读取小说信息");
    res.json({
      novelId: novel.novelId,
      novelName: novel.novelName,
      authorName: novel.authorName,
      novelCover: novel.novelCover,
      lastUpdateTime: novel.lastUpdateTime,
      description: novel.expand?.intro || "暂无简介",
      isFinish: novel.isFinish,
      typeName: novel.expand?.typeName,
    });
  } catch (error) {
    res.status(500).json({
      message: error instanceof Error ? error.message : "读取小说信息失败",
    });
  }
});

app.get("/api/audio/:novelId", async (req, res) => {
  const session = getAuthSession(req);
  if (!session)
    return res.status(401).json({ message: "有声内容需要登录 SF 账号" });
  const novelId = Number(req.params.novelId);
  if (!Number.isInteger(novelId) || novelId <= 0)
    return res.status(400).json({ message: "小说编号无效" });
  try {
    const audio = await cached(
      sessionKey(session.cookie, `audio:${novelId}`),
      audioCacheTtl,
      () => throttledMetadata(() => getAudioChapters(novelId, session.cookie)),
    );
    const localState = await getLocalDownloadState(novelId);
    const downloaded = new Set(localState.downloadedAudioChapterIds);
    res.json({
      title: audio.title,
      chapters: audio.chapters.map(({ id, title, volume }) => ({
        id,
        title,
        volume,
        downloaded: downloaded.has(id),
      })),
    });
  } catch (error) {
    if (error instanceof AudioCatalogError) {
      // No audio is a normal availability result, so keep it out of the
      // browser's failed-resource console while still exposing a code.
      if (error.code === "NO_AUDIO")
        return res.json({
          code: error.code,
          message: error.message,
          chapters: [],
        });
      return res
        .status(error.httpStatus)
        .json({ code: error.code, message: error.message });
    }
    res.status(500).json({
      code: "AUDIO_UNKNOWN_ERROR",
      message: error instanceof Error ? error.message : "读取有声目录失败",
    });
  }
});

app.get("/api/auth/status", (req, res) => {
  const session = getAuthSession(req);
  res.json({ authenticated: Boolean(session), userName: session?.userName });
});

app.post("/api/auth/browser-login", async (_req, res) => {
  if (!(await isControlledBrowserRunning())) {
    const executable = findBrowserExecutable();
    if (!executable)
      return res
        .status(500)
        .json({ message: "未找到 Microsoft Edge 或 Google Chrome" });
    await fse.ensureDir(browserProfileDir);
    const browser = spawn(
      executable,
      [
        `--remote-debugging-port=${browserDebugPort}`,
        "--remote-debugging-address=127.0.0.1",
        "--remote-allow-origins=*",
        `--user-data-dir=${browserProfileDir}`,
        "--no-first-run",
        "--no-default-browser-check",
        "--new-window",
        browserLoginUrl,
      ],
      { detached: true, stdio: "ignore", windowsHide: false },
    );
    browser.unref();
    browserLaunchPendingUntil = Date.now() + 12_000;
  }
  res.status(202).json({ status: "waiting" });
});

app.get("/api/auth/browser-login/status", async (_req, res) => {
  const session = await getControlledBrowserSession();
  if (!session)
    return res.json({
      authenticated: false,
      waiting:
        (await isControlledBrowserRunning()) ||
        Date.now() < browserLaunchPendingUntil,
    });
  if (!session.loginCompleted)
    return res.json({
      authenticated: false,
      waiting:
        (await isControlledBrowserRunning()) ||
        Date.now() < browserLaunchPendingUntil,
    });
  saveAuthSession(res, { cookie: session.cookie, userName: "已登录 SF 账号" });
  browserLaunchPendingUntil = 0;
  await closeControlledBrowser();
  res.json({ authenticated: true, userName: "已登录 SF 账号" });
});

app.post("/api/auth/logout", (_req, res) => {
  res.clearCookie(localAuthCookie, {
    httpOnly: true,
    sameSite: "strict",
    path: "/",
  });
  res.status(204).end();
});

function runJob(job: Job) {
  const session = jobSessions.get(job.id);
  if (!session) return;
  const controller = new AbortController();
  jobControllers.set(job.id, controller);
  void (async () => {
    try {
      job.status = "downloading";
      if (job.kind === "audio") {
        const saved = await writeAudio(
          job.novelId,
          session.cookie || "",
          job.chapterIds,
          controller.signal,
          (progress, message) => Object.assign(job, { progress, message }),
        );
        Object.assign(job, {
          status: "done",
          progress: 100,
          message: `已保存 ${saved.chapters} 集有声内容`,
          file: bookUrl(saved.folder, saved.file),
        });
      } else {
        const saved = await writeNovel(
          job.novelId,
          session.cookie,
          job.chapterIds,
          controller.signal,
          (progress, message) => Object.assign(job, { progress, message }),
        );
        Object.assign(job, {
          status: "done",
          progress: 100,
          message: `已保存 ${saved.chapters} 个章节`,
          file: bookUrl(saved.folder, saved.file),
        });
      }
    } catch (error) {
      if (controller.signal.aborted && job.status === "paused")
        job.message = "已暂停，可继续下载";
      else if (controller.signal.aborted)
        Object.assign(job, { status: "cancelled", message: "下载已取消" });
      else
        Object.assign(job, {
          status: "error",
          message: error instanceof Error ? error.message : "下载失败",
        });
    } finally {
      jobControllers.delete(job.id);
    }
  })();
}

function validateChapterIds(chapterIds: unknown) {
  return (
    chapterIds === undefined ||
    (Array.isArray(chapterIds) &&
      chapterIds.every((id) => Number.isInteger(id) && id > 0))
  );
}

app.post("/api/download", (req, res) => {
  const { novelId, title, chapterIds } = req.body as {
    novelId?: number;
    title?: string;
    chapterIds?: number[];
  };
  if (!novelId) return res.status(400).json({ message: "缺少小说编号" });
  if (!validateChapterIds(chapterIds))
    return res.status(400).json({ message: "章节选择无效" });
  const id = `${Date.now()}-${novelId}`;
  const job: Job = {
    id,
    title: title || `小说 ${novelId}`,
    kind: "text",
    novelId,
    chapterIds,
    status: "queued",
    progress: 0,
    message: "等待开始",
  };
  jobs.set(id, job);
  jobSessions.set(id, { cookie: getAuthSession(req)?.cookie, chapterIds });
  runJob(job);
  res.status(202).json(job);
});

app.post("/api/audio/download", (req, res) => {
  const { novelId, title, chapterIds } = req.body as {
    novelId?: number;
    title?: string;
    chapterIds?: number[];
  };
  const session = getAuthSession(req);
  if (!session)
    return res.status(401).json({ message: "有声内容需要登录 SF 账号" });
  if (!novelId) return res.status(400).json({ message: "缺少小说编号" });
  if (!validateChapterIds(chapterIds))
    return res.status(400).json({ message: "章节选择无效" });
  const id = `${Date.now()}-audio-${novelId}`;
  const job: Job = {
    id,
    title: title || `小说 ${novelId}`,
    kind: "audio",
    novelId,
    chapterIds,
    status: "queued",
    progress: 0,
    message: "等待开始",
  };
  jobs.set(id, job);
  jobSessions.set(id, { cookie: session.cookie, chapterIds });
  runJob(job);
  res.status(202).json(job);
});

app.post("/api/jobs/:id/pause", (req, res) => {
  const job = jobs.get(req.params.id);
  if (!job) return res.status(404).json({ message: "下载任务不存在" });
  if (job.status === "queued" || job.status === "downloading") {
    job.status = "paused";
    job.message = "正在暂停下载";
    jobControllers.get(job.id)?.abort();
  }
  res.json(job);
});

app.post("/api/jobs/:id/resume", (req, res) => {
  const job = jobs.get(req.params.id);
  if (!job) return res.status(404).json({ message: "下载任务不存在" });
  if (job.status === "paused") {
    job.message = "等待继续";
    runJob(job);
  }
  res.json(job);
});

app.post("/api/jobs/:id/cancel", (req, res) => {
  const job = jobs.get(req.params.id);
  if (!job) return res.status(404).json({ message: "下载任务不存在" });
  if (
    job.status === "done" ||
    job.status === "error" ||
    job.status === "cancelled"
  )
    return res.json(job);
  job.status = "cancelled";
  job.message = "正在停止下载";
  jobControllers.get(job.id)?.abort();
  res.json(job);
});

app.delete("/api/jobs/:id", (req, res) => {
  const job = jobs.get(req.params.id);
  if (!job) return res.status(404).json({ message: "下载任务不存在" });
  jobControllers.get(job.id)?.abort();
  jobs.delete(job.id);
  jobControllers.delete(job.id);
  jobSessions.delete(job.id);
  res.status(204).end();
});

app.get("/api/jobs", (_req, res) => res.json([...jobs.values()].reverse()));

app.get("/api/library", async (_req, res) => {
  await fse.ensureDir(libraryDir);
  const folders = await fse.readdir(libraryDir, { withFileTypes: true });
  const books = await Promise.all(
    folders
      .filter((folder) => folder.isDirectory())
      .map(async (folder) => {
        const dir = path.join(libraryDir, folder.name);
        const files = await fse.readdir(dir);
        const markdown = files.find((file) => file.endsWith(".md"));
        const audioPlaylist = path.join(dir, "audio", "有声目录.m3u8");
        const hasAudio = await fse.pathExists(audioPlaylist);
        if (!markdown && !hasAudio) return null;
        const updatedFile = markdown ? path.join(dir, markdown) : audioPlaylist;
        const cover = path.join(dir, "imgs", "cover.jpeg");
        let metadata: { novelId?: number } = {};
        try {
          metadata = await fse.readJson(path.join(dir, ".novel-flow.json"));
        } catch {
          /* metadata is optional for older downloads */
        }
        return {
          name: folder.name,
          novelId: Number.isInteger(metadata.novelId)
            ? metadata.novelId
            : undefined,
          href: markdown ? bookUrl(folder.name, markdown) : undefined,
          audioHref: hasAudio
            ? bookUrl(folder.name, path.posix.join("audio", "有声目录.m3u8"))
            : undefined,
          cover: (await fse.pathExists(cover))
            ? bookUrl(folder.name, path.posix.join("imgs", "cover.jpeg"))
            : undefined,
          updatedAt: (await fse.stat(updatedFile)).mtime,
        };
      }),
  );
  res.json(books.filter(Boolean));
});

app.delete("/api/library/:folder", async (req, res) => {
  const folder = safeName(decodeURIComponent(req.params.folder));
  const target = path.resolve(libraryDir, folder);
  const base = path.resolve(libraryDir);
  if (target === base || !target.startsWith(`${base}${path.sep}`))
    return res.status(400).json({ message: "书籍目录无效" });
  if (!(await fse.pathExists(target)))
    return res.status(404).json({ message: "本地书籍不存在" });
  await fse.remove(target);
  res.status(204).end();
});

app.get("/api/health", (_req, res) => res.json({ ok: true }));

// Production mode serves the Vue build from the same local server.
app.use(express.static(webDir));

app.listen(8787, "127.0.0.1", () =>
  console.log("Novel Flow API: http://127.0.0.1:8787"),
);
