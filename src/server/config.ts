import path from "node:path";

const rootDir = process.cwd();

export const config = {
  rootDir,
  libraryDir: path.join(rootDir, "output", "菠萝包轻小说"),
  webDir: path.join(rootDir, "dist"),
  port: 8787,
  host: "127.0.0.1",
  auth: {
    cookieName: "sfacg_session",
    cookieMaxAge: 30 * 24 * 60 * 60 * 1000,
    browserDebugPort: 9223,
    browserProfileDir: path.join(rootDir, ".sfacg-login-profile"),
    browserLoginUrl: "https://passport.sfacg.com/Login.aspx",
  },
  cache: {
    metadataTtl: 60_000,
    audioTtl: 120_000,
    bookshelfTtl: 120_000,
  },
} as const;
