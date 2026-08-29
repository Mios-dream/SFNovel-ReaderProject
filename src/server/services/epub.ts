import fse from "fs-extra";
import path from "node:path";
import type { StoredTextChapter } from "../types";

type EpubBook = {
  title: string;
  author: string;
  description: string;
  chapters: StoredTextChapter[];
  coverPath?: string;
};

type ZipEntry = { name: string; data: Buffer; mimeType?: string };

function xml(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&apos;");
}

function htmlFromMarkdown(value: string) {
  const body = value.replace(/^##\s+.+\r?\n+/, "").trim();
  return body
    .split(/\r?\n\s*\r?\n/)
    .filter(Boolean)
    .map((paragraph) => `<p>${xml(paragraph).replaceAll("\n", "<br/>")}</p>`)
    .join("\n");
}

function crc32(data: Buffer) {
  let crc = 0xffffffff;
  for (const byte of data) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1)
      crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function uint16(value: number) {
  const result = Buffer.alloc(2);
  result.writeUInt16LE(value, 0);
  return result;
}

function uint32(value: number) {
  const result = Buffer.alloc(4);
  result.writeUInt32LE(value >>> 0, 0);
  return result;
}

/** 生成无压缩 ZIP；EPUB 阅读器要求 mimetype 是归档中的第一项且不压缩。 */
function zip(entries: ZipEntry[]) {
  const chunks: Buffer[] = [];
  const central: Buffer[] = [];
  let offset = 0;
  for (const entry of entries) {
    const name = Buffer.from(entry.name);
    const crc = crc32(entry.data);
    const local = Buffer.concat([
      uint32(0x04034b50), uint16(20), uint16(0), uint16(0), uint16(0), uint16(0),
      uint32(crc), uint32(entry.data.length), uint32(entry.data.length), uint16(name.length), uint16(0), name,
    ]);
    chunks.push(local, entry.data);
    central.push(Buffer.concat([
      uint32(0x02014b50), uint16(20), uint16(20), uint16(0), uint16(0), uint16(0), uint16(0),
      uint32(crc), uint32(entry.data.length), uint32(entry.data.length), uint16(name.length), uint16(0), uint16(0),
      uint16(0), uint16(0), uint32(0), uint32(offset), name,
    ]));
    offset += local.length + entry.data.length;
  }
  const centralData = Buffer.concat(central);
  return Buffer.concat([
    ...chunks,
    centralData,
    uint32(0x06054b50), uint16(0), uint16(0), uint16(entries.length), uint16(entries.length),
    uint32(centralData.length), uint32(offset), uint16(0),
  ]);
}

/** 从本地已下载章节生成 EPUB 3 文件。 */
export async function writeEpub(target: string, book: EpubBook) {
  if (!book.chapters.length) throw new Error("尚无可导出的文字章节");
  const chapterEntries = book.chapters.map((chapter, index) => ({
    id: `chapter-${index + 1}`,
    file: `chapter-${String(index + 1).padStart(4, "0")}.xhtml`,
    title: chapter.title,
    data: `<?xml version="1.0" encoding="utf-8"?>\n<!DOCTYPE html><html xmlns="http://www.w3.org/1999/xhtml" xml:lang="zh-Hans"><head><title>${xml(chapter.title)}</title><meta charset="utf-8"/></head><body><h1>${xml(chapter.title)}</h1>${htmlFromMarkdown(chapter.content)}</body></html>`,
  }));
  const entries: ZipEntry[] = [
    { name: "mimetype", data: Buffer.from("application/epub+zip") },
    { name: "META-INF/container.xml", data: Buffer.from(`<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>`) },
  ];
  let coverManifest = "";
  if (book.coverPath && (await fse.pathExists(book.coverPath))) {
    entries.push({ name: "OEBPS/cover.jpeg", data: await fse.readFile(book.coverPath) });
    coverManifest = '<item id="cover-image" href="cover.jpeg" media-type="image/jpeg" properties="cover-image"/>';
  }
  entries.push({
    name: "OEBPS/nav.xhtml",
    data: Buffer.from(`<?xml version="1.0" encoding="utf-8"?><!DOCTYPE html><html xmlns="http://www.w3.org/1999/xhtml" xml:lang="zh-Hans"><head><title>目录</title><meta charset="utf-8"/></head><body><nav epub:type="toc" xmlns:epub="http://www.idpf.org/2007/ops"><h1>目录</h1><ol>${chapterEntries.map((chapter) => `<li><a href="${chapter.file}">${xml(chapter.title)}</a></li>`).join("")}</ol></nav></body></html>`),
  });
  entries.push(...chapterEntries.map((chapter) => ({ name: `OEBPS/${chapter.file}`, data: Buffer.from(chapter.data) })));
  entries.push({
    name: "OEBPS/content.opf",
    data: Buffer.from(`<?xml version="1.0" encoding="utf-8"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="book-id" xml:lang="zh-Hans"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="book-id">urn:uuid:local-${Date.now()}</dc:identifier><dc:title>${xml(book.title)}</dc:title><dc:creator>${xml(book.author || "未知作者")}</dc:creator><dc:language>zh-Hans</dc:language><dc:description>${xml(book.description)}</dc:description></metadata><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>${coverManifest}${chapterEntries.map((chapter) => `<item id="${chapter.id}" href="${chapter.file}" media-type="application/xhtml+xml"/>`).join("")}</manifest><spine>${chapterEntries.map((chapter) => `<itemref idref="${chapter.id}"/>`).join("")}</spine></package>`),
  });
  await fse.outputFile(target, zip(entries));
}
