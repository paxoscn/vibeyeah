---
name: feishu-group-members
description: Query Feishu/Lark group chat members via API — get tenant token, list members with names and Open IDs.
tags: [feishu, lark, api, group, members]
triggers:
  - user asks to list group members
  - user asks who is in a Feishu group
  - user needs member Open IDs for a chat
  - 查询群成员 / 群里有谁
---

# Feishu Group Members Query

Query members of a Feishu/Lark group chat using the Open API.

## Prerequisites

- A Feishu app with `app_id` and `app_secret` (from Feishu Open Platform console)
- The app must have `im:chat:readonly` or equivalent permission
- The `chat_id` of the target group (available from message context or chat URL)

## Step 1: Get Tenant Access Token

```bash
curl -s -X POST 'https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal' \
  -H 'Content-Type: application/json' \
  -d '{
    "app_id": "YOUR_APP_ID",
    "app_secret": "YOUR_APP_SECRET"
  }'
```

Response contains `tenant_access_token` — extract it for the next step.

Using Python:
```python
import requests

resp = requests.post(
    "https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal",
    json={"app_id": APP_ID, "app_secret": APP_SECRET}
)
token = resp.json()["tenant_access_token"]
```

## Step 2: List Group Members

```bash
curl -s -X GET "https://open.feishu.cn/open-apis/im/v1/chats/{chat_id}/members" \
  -H "Authorization: Bearer {tenant_access_token}"
```

Using Python:
```python
resp = requests.get(
    f"https://open.feishu.cn/open-apis/im/v1/chats/{chat_id}/members",
    headers={"Authorization": f"Bearer {token}"}
)
members = resp.json()["data"]["items"]
for m in members:
    print(f"{m['name']} — {m['member_id']}")
```

## Response Structure

```json
{
  "code": 0,
  "data": {
    "items": [
      {
        "member_id_type": "open_id",
        "member_id": "ou_xxxxx",
        "name": "张三",
        "tenant_key": "..."
      }
    ],
    "page_token": "...",
    "has_more": false
  }
}
```

Key fields per member:
- `name` — display name
- `member_id` — Open ID (format: `ou_xxxxx`)
- `member_id_type` — usually `open_id`

## Pagination

For large groups (>100 members), use `page_token` from response:

```bash
curl -s -X GET "https://open.feishu.cn/open-apis/im/v1/chats/{chat_id}/members?page_token={page_token}&page_size=100" \
  -H "Authorization: Bearer {tenant_access_token}"
```

Loop until `has_more` is `false`.

## Getting chat_id

In Hermes Feishu sessions, the `chat_id` is available in the session context header:
```
Source: Feishu (group: 群名. ..., thread: ...)
```

The actual `chat_id` (format: `oc_xxxxx`) can be extracted from the Feishu message event payload or from the group's URL.

## Pitfalls

- Token expires after ~2 hours — re-fetch if you get auth errors
- The app bot must be a member of the group to query its members
- `member_id` format depends on `member_id_type` param (default: `open_id`)
- Some fields require additional scopes (e.g., email, phone need `contact:user.base:readonly`)

## Example Output

```markdown
| # | 姓名 | Open ID |
|---|------|---------|
| 1 | 何旭 | `ou_bf59e05882832d116be634424ceff213` |
| 2 | 莫日根 | `ou_b0739f45bcc7865c761e4b8239b5a8a8` |
| 3 | 陈柏君 | `ou_7efda0883804867f8a843f47edfa71ac` |
```
