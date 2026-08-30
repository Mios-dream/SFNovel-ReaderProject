import fse from "fs-extra";
import defaultDictionary from "../infrastructure/sfacg/content-dictionary.json";
import { config } from "../config";
import { SfacgApiClient } from "../infrastructure/sfacg/client";

type ContentDictionary = Record<string, string>;

const hanPattern = /[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff]/u;
let dictionary: ContentDictionary = {};
let loaded = false;

function validDictionary(value: unknown): ContentDictionary {
  if (!value || typeof value !== "object" || Array.isArray(value)) return {};
  return Object.fromEntries(
    Object.entries(value).filter(
      ([key, replacement]) =>
        [...key].length === 1 &&
        [...replacement].length === 1 &&
        hanPattern.test(key) &&
        hanPattern.test(replacement),
    ),
  );
}

/** 读取可由用户更新的映射表；首次运行从 Oevani 初始表复制到配置目录。 */
export async function loadSfacgContentDictionary() {
  if (loaded) return dictionary;
  try {
    dictionary = validDictionary(await fse.readJson(config.sfacgDictionaryFile));
  } catch {
    dictionary = validDictionary(defaultDictionary);
    await fse.outputJson(config.sfacgDictionaryFile, dictionary, { spaces: 2 });
  }
  loaded = true;
  return dictionary;
}

export function getSfacgContentDictionarySize() {
  return Object.keys(dictionary).length;
}

/** 只替换 API 正文中的混淆汉字，标点、换行和图片标记保持不变。 */
export function decodeSfacgContent(content: string) {
  return [...content].map((character) => dictionary[character] || character).join("");
}

function hanCharacters(content: string) {
  return [...content].filter((character) => hanPattern.test(character));
}

/** 用同一公开章节的 API 与网页正文建立或补充字符映射。 */
export async function updateSfacgContentDictionary(
  chapterId: number,
  client = new SfacgApiClient(),
) {
  await loadSfacgContentDictionary();
  const api = await client.chapterContentWithMetadataFromApi(chapterId);
  const web = await client.chapterContentFromWeb(api.novelId, api.volumeId, chapterId);
  const source = hanCharacters(api.content);
  const target = hanCharacters(web);
  if (!source.length || source.length !== target.length)
    throw new Error(`API 与网页正文无法对齐（API ${source.length} 字，网页 ${target.length} 字）`);

  const previousKeys = new Set(Object.keys(dictionary));
  const additions: ContentDictionary = {};
  for (let index = 0; index < source.length; index += 1) {
    const from = source[index];
    const to = target[index];
    const existing = dictionary[from] || additions[from];
    if (existing && existing !== to)
      throw new Error(`映射冲突：${from} 已映射为 ${existing}，网页对应 ${to}`);
    additions[from] = to;
  }
  const next = { ...dictionary, ...additions };
  dictionary = next;
  await fse.outputJson(config.sfacgDictionaryFile, next, { spaces: 2 });
  return {
    size: Object.keys(next).length,
    added: Object.keys(additions).filter((key) => !previousKeys.has(key)).length,
  };
}

export function resetSfacgContentDictionaryForTests() {
  loaded = false;
  dictionary = {};
}
