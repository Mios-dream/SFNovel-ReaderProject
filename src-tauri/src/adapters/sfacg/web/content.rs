use super::html::html_tag_attribute;
use super::placeholder::is_vip_subscription_placeholder;
use super::WebClient;
use crate::domain::ids::validate_novel_id;

/// VIP 章节返回的已校验图片。
pub(crate) struct VipChapterImage {
    /// 已校验格式的图片二进制内容。
    pub(crate) bytes: Vec<u8>,
    /// 根据文件签名识别出的图片扩展名。
    pub(crate) extension: &'static str,
}

impl WebClient {
    /// 读取普通网页文字章节并转换为可持久化的文本格式。
    ///
    /// 保留章节正文中的远程图片标记，供后续下载器解析。
    ///
    /// # 参数
    /// - `novel_id`：作品编号，用于构造章节地址和 Referer。
    /// - `volume_id`：分卷编号，用于构造章节地址。
    /// - `chapter_id`：章节编号。
    ///
    /// # 返回
    /// 返回规范化换行并去除首尾空白的章节正文。
    ///
    /// # 错误
    /// 页面请求失败、缺少正文容器或正文为空时返回错误。
    pub(crate) async fn chapter_content(
        &self,
        novel_id: i64,
        volume_id: i64,
        chapter_id: i64,
    ) -> Result<String, String> {
        let html = self
            .page_request(
                format!("https://book.sfacg.com/Novel/{novel_id}/{volume_id}/{chapter_id}/"),
                Some(&format!(
                    "https://book.sfacg.com/Novel/{novel_id}/MainIndex/"
                )),
            )
            .send()
            .await
            .map_err(|error| format!("无法读取 SF 网页章节：{error}"))?
            .error_for_status()
            .map_err(|error| format!("SF 网页章节请求被拒绝：{error}"))?
            .text()
            .await
            .map_err(|error| format!("SF 网页章节格式无效：{error}"))?;
        let start = html
            .find("id=\"ChapterBody\"")
            .or_else(|| html.find("id='ChapterBody'"))
            .ok_or_else(|| "SF 网页响应中没有找到 #ChapterBody".to_string())?;
        let body_start = html[start..]
            .find('>')
            .map(|offset| start + offset + 1)
            .ok_or_else(|| "SF 章节正文结构无效".to_string())?;
        let body_end = html[body_start..]
            .find("</div>")
            .map(|offset| body_start + offset)
            .ok_or_else(|| "SF 章节正文结构无效".to_string())?;
        let body = html[body_start..body_end]
            .replace("<br>", "\n")
            .replace("<br/>", "\n")
            .replace("<br />", "\n")
            .replace("</p>", "\n")
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">");
        let plain = chapter_html_to_markdown(&body);
        let plain = plain.replace("\r\n", "\n").replace('\r', "\n");
        if plain.trim().is_empty() {
            return Err("SF 网页章节正文为空".to_string());
        }
        Ok(plain.trim().to_string())
    }

    /// 请求已授权 VIP 文字章节的图片内容。
    ///
    /// 请求刻意限制为原生 Web 会话。仅有成功 HTTP 状态或图片 MIME 类型不足以证明可用，
    /// 因为上游会以 `200 image/png` 返回订阅提示图。
    pub(crate) async fn vip_chapter_image(
        &self,
        novel_id: i64,
        chapter_id: i64,
    ) -> Result<VipChapterImage, String> {
        validate_novel_id(novel_id)?;
        if chapter_id <= 0 {
            return Err("VIP 章节参数无效".to_string());
        }
        self.require_session()?;
        let referer = format!("https://book.sfacg.com/vip/c/{chapter_id}/");
        let response = self
            .ajax_request("https://book.sfacg.com/ajax/ashx/common.ashx", &referer)
            .query(&[
                ("op", "getChapPic"),
                ("tp", "true"),
                ("quick", "true"),
                ("cid", &chapter_id.to_string()),
                ("nid", &novel_id.to_string()),
                ("font", "18"),
                ("lang", ""),
                ("w", "728"),
            ])
            .header(
                reqwest::header::ACCEPT,
                "image/avif,image/webp,image/apng,image/*,*/*;q=0.8",
            )
            .send()
            .await
            .map_err(|error| format!("无法读取 VIP 章节图片：{error}"))?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            || response.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err("网页登录会话已失效".to_string());
        }
        let response = response
            .error_for_status()
            .map_err(|error| format!("VIP 章节图片请求被拒绝：{error}"))?;
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        let bytes = response
            .bytes()
            .await
            .map_err(|error| format!("无法读取 VIP 章节图片数据：{error}"))?
            .to_vec();
        let extension = if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            "gif"
        } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
            "png"
        } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            "jpg"
        } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
            "webp"
        } else {
            return Err(if content_type.contains("text/html") {
                "VIP 章节未返回图片内容，请确认网页登录会话和购买状态".to_string()
            } else {
                "VIP 章节资源格式无效".to_string()
            });
        };
        if bytes.len() < 512 {
            return Err("VIP 章节图片数据无效或为空".to_string());
        }
        if is_vip_subscription_placeholder(&bytes) {
            return Err("VIP 章节未授权访问：网页返回了订阅提示图，请确认本章已订阅".to_string());
        }
        Ok(VipChapterImage { bytes, extension })
    }
}

/// 将章节 HTML 中的图片保留为下载器可识别的 SF 图片标记。
fn chapter_html_to_markdown(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(tag_start) = remaining.find('<') {
        output.push_str(&remaining[..tag_start]);
        let after_start = &remaining[tag_start..];
        let Some(tag_end) = after_start.find('>') else {
            output.push_str(after_start);
            return output;
        };
        let tag = &after_start[..=tag_end];
        let normalized = tag.to_ascii_lowercase();
        if normalized.starts_with("<img") {
            if let Some(source) = html_tag_attribute(tag, "src") {
                let source = source
                    .strip_prefix("//")
                    .map_or_else(|| source.to_string(), |value| format!("https://{value}"));
                if source.starts_with("https://") || source.starts_with("http://") {
                    output.push_str(&format!("\n[img=]{}[/img]\n", source));
                }
            }
        }
        remaining = &after_start[tag_end + 1..];
    }
    output.push_str(remaining);
    output
}

#[cfg(test)]
mod tests {
    use super::chapter_html_to_markdown;

    #[test]
    fn keeps_remote_chapter_images_as_download_markers() {
        assert_eq!(
            chapter_html_to_markdown(
                "前文<img src=\"//rss.sfacg.com/body/1.png\">后文<img src='https://rss.sfacg.com/body/2.jpg'/>"
            ),
            "前文\n[img=]https://rss.sfacg.com/body/1.png[/img]\n后文\n[img=]https://rss.sfacg.com/body/2.jpg[/img]\n"
        );
    }

    #[test]
    fn drops_images_that_are_not_remote_http_resources() {
        assert_eq!(
            chapter_html_to_markdown("甲<img src=\"data:image/png;base64,abc\">乙"),
            "甲乙"
        );
    }
}
