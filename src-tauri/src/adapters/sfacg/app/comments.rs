use super::error::AppApiError;
use super::parsing::{json_bool_field, json_i64_field, json_text_field};
use super::AppClient;
use crate::domain::catalog::{
    BookComment, BookCommentPage, BookCommentReply, BookCommentReplyPage,
};
use serde_json::Value;

impl AppClient {
    /// 读取小说、漫画或有声作品的公开评论。
    pub(crate) async fn get_book_comments(
        &self,
        kind: &str,
        novel_id: i64,
        media_id: Option<i64>,
        page: i64,
        size: i64,
        reviews: bool,
    ) -> Result<BookCommentPage, AppApiError> {
        if reviews && kind != "novel" {
            return Ok(BookCommentPage {
                comments: Vec::new(),
                highlights: Vec::new(),
                page,
                has_more: false,
            });
        }
        let path = match kind {
            "novel" if reviews => format!("/novels/{novel_id}/lcmts"),
            "novel" => format!("/novels/{novel_id}/Cmts"),
            "audio" => format!(
                "/albums/{}/albumcmts",
                media_id.ok_or_else(|| AppApiError::local("有声作品缺少专辑编号".to_string()))?
            ),
            "comic" => format!("/comics/{novel_id}/comiccmts"),
            _ => return Err(AppApiError::local("不支持的评论类型".to_string())),
        };
        let mut comments = Vec::new();
        let mut highlights: Vec<BookComment> = Vec::new();
        let mut source_page = page;
        let mut has_more = false;
        for _ in 0..10 {
            let response = self
                .get_public_data(
                    &path,
                    &[
                        ("page", source_page.to_string()),
                        ("size", size.to_string()),
                    ],
                )
                .await?;
            let items = response.as_array().cloned().unwrap_or_default();
            let received = items.len();
            for item in &items {
                let Some(comment) = book_comment(item) else {
                    continue;
                };
                if !reviews && comment.ext_type != 0 {
                    if highlights.len() < 12
                        && highlights
                            .iter()
                            .all(|item| item.comment_id != comment.comment_id)
                    {
                        highlights.push(comment);
                    }
                } else {
                    comments.push(comment);
                }
            }
            has_more = received == size as usize;
            source_page += 1;
            if reviews || comments.len() >= size as usize || !has_more {
                break;
            }
        }
        Ok(BookCommentPage {
            has_more,
            comments,
            highlights,
            page: source_page - 1,
        })
    }

    /// 读取一条评论的回复。
    pub(crate) async fn get_comment_replies(
        &self,
        kind: &str,
        comment_id: i64,
        reviews: bool,
        page: i64,
        size: i64,
    ) -> Result<BookCommentReplyPage, AppApiError> {
        if reviews && kind != "novel" {
            return Ok(BookCommentReplyPage {
                replies: Vec::new(),
                page,
                has_more: false,
            });
        }
        let path = match kind {
            "novel" if reviews => format!("/lcmts/{comment_id}/replys"),
            "novel" => format!("/cmts/{comment_id}/replys"),
            "audio" => format!("/albumcmts/{comment_id}/replys"),
            "comic" => format!("/comiccmts/{comment_id}/replys"),
            _ => return Err(AppApiError::local("不支持的评论类型".to_string())),
        };
        let response = self
            .get_public_data(
                &path,
                &[("page", page.to_string()), ("size", size.to_string())],
            )
            .await?;
        let items = response
            .as_array()
            .or_else(|| response.get("list").and_then(Value::as_array))
            .or_else(|| response.get("replys").and_then(Value::as_array))
            .or_else(|| response.get("replies").and_then(Value::as_array))
            .cloned()
            .unwrap_or_default();
        let has_more = items.len() == size as usize;
        Ok(BookCommentReplyPage {
            replies: items.iter().filter_map(book_comment_reply).collect(),
            page,
            has_more,
        })
    }
}

/// 将公开评论对象转换为渲染层记录。
fn book_comment(item: &Value) -> Option<BookComment> {
    Some(BookComment {
        comment_id: json_i64_field(item, &["commentId", "lcmtId", "id"]).filter(|id| *id > 0)?,
        title: json_text_field(item, &["title", "subject"]).unwrap_or_default(),
        user_name: json_text_field(item, &["displayName", "nickName", "userName", "posterName"])
            .unwrap_or_else(|| "SF 读者".to_string()),
        avatar: json_text_field(item, &["avatar", "userAvatar"]).unwrap_or_default(),
        content: json_text_field(item, &["content", "intro"])?,
        created_at: json_text_field(item, &["createTime", "postTime"]).unwrap_or_default(),
        reply_count: json_i64_field(item, &["replyNum", "replyCount"]).unwrap_or(0),
        like_count: json_i64_field(item, &["favCount", "likeCount"]).unwrap_or(0),
        role_name: json_text_field(item, &["roleName"]).unwrap_or_default(),
        is_stick: json_bool_field(item, &["isStick"]).unwrap_or(false),
        ext_type: json_i64_field(item, &["extType"]).unwrap_or(0),
        ticket_num: json_i64_field(item, &["ticketNum"]).unwrap_or(0),
    })
}

/// 将评论回复对象转换为渲染层记录。
fn book_comment_reply(item: &Value) -> Option<BookCommentReply> {
    Some(BookCommentReply {
        reply_id: json_i64_field(item, &["replyId", "cmtReplyId", "commentReplyId", "id"])
            .filter(|id| *id > 0)?,
        user_name: json_text_field(item, &["displayName", "nickName", "userName", "posterName"])
            .unwrap_or_else(|| "SF 读者".to_string()),
        avatar: json_text_field(item, &["avatar", "userAvatar"]).unwrap_or_default(),
        content: json_text_field(item, &["content", "replyContent", "intro"])?,
        created_at: json_text_field(item, &["createTime", "postTime", "replyTime"])
            .unwrap_or_default(),
        like_count: json_i64_field(item, &["favCount", "likeCount"]).unwrap_or(0),
        replied_user_name: json_text_field(
            item,
            &[
                "repliedUserName",
                "repliedName",
                "replyToUserName",
                "toUserName",
            ],
        )
        .unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::{book_comment, book_comment_reply};
    use serde_json::json;

    #[test]
    fn comment_requires_positive_id_and_content() {
        assert!(book_comment(&json!({ "commentId": 0, "content": "正文" })).is_none());
        assert!(
            book_comment(&json!({ "commentId": 1 })).is_none(),
            "缺少正文的评论应跳过"
        );

        let comment = book_comment(&json!({ "commentId": 1, "content": "正文" })).expect("应解析");
        assert_eq!(comment.comment_id, 1);
        assert_eq!(comment.user_name, "SF 读者", "缺少昵称时使用占位名");
    }

    #[test]
    fn comment_keeps_interaction_fields() {
        let comment = book_comment(&json!({
            "lcmtId": 9,
            "content": "月票",
            "extType": 2,
            "ticketNum": 5,
            "isStick": true
        }))
        .expect("应解析");

        assert_eq!(comment.comment_id, 9, "应接受 lcmtId 作为评论编号");
        assert_eq!(comment.ext_type, 2);
        assert_eq!(comment.ticket_num, 5);
        assert!(comment.is_stick);
    }

    #[test]
    fn reply_requires_positive_id_and_content() {
        assert!(book_comment_reply(&json!({ "replyId": 0, "content": "回复" })).is_none());
        assert!(book_comment_reply(&json!({ "replyId": 2 })).is_none());

        let reply =
            book_comment_reply(&json!({ "replyId": 2, "replyContent": "回复" })).expect("应解析");
        assert_eq!(reply.reply_id, 2);
        assert_eq!(reply.content, "回复", "应接受 replyContent 作为正文");
    }
}
