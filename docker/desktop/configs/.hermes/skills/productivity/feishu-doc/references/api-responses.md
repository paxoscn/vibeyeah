# Feishu Open API — Response Structures Observed

## tenant_access_token Response

```json
{
  "code": 0,
  "expire": 6785,
  "msg": "ok",
  "tenant_access_token": "t-g1047..."
}
```

## Wiki Node Resolution Response

```json
{
  "code": 0,
  "data": {
    "node": {
      "creator": "ou_80ce...",
      "node_token": "DocTokenPlaceholderxxxxxxxxx",
      "node_type": "origin",
      "obj_token": "ObjTokenPlaceholderxxxxxxxxx",
      "obj_type": "docx",
      "origin_node_token": "DocTokenPlaceholderxxxxxxxxx",
      "origin_space_id": "7529468953365168156",
      "owner": "ou_80ce...",
      "space_id": "7529468953365168156",
      "title": "MDM/GTM服务QPS/内存占用优化步骤"
    }
  },
  "msg": "success"
}
```

Key fields:
- `obj_token` — use this for all document content APIs
- `obj_type` — "docx" for modern docs, "doc" for legacy
- `title` — document title

## Document Raw Content Response

```json
{
  "code": 0,
  "data": {
    "content": "Full document text..."
  },
  "msg": "success"
}
```

The content is plain text. Tables are represented inline with cell values separated by newlines. Headings appear as text without markdown markers.

## URL → Token Extraction Pattern

From a Feishu wiki URL:
```
https://{tenant}.feishu.cn/wiki/{node_token}
```

From a Feishu doc URL:
```
https://{tenant}.feishu.cn/docx/{doc_token}
```

The `node_token` or `doc_token` is the last path segment (before any query string).
