---
format: aep.planning-md/3
id: story:pager-last-page
kind: story
status: active
title: Pagination drops the last page
summary: With 11 items and 5 items per page, pager::page_count(11, 5) returns 2 and pager::page(&items, 3, 5) returns an empty slice, so item 11 is never shown
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-22T11:55:28Z", actor: "agent:bugfix.seed", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-22T11:55:28Z", actor: "agent:bugfix.seed", revision: 3}
---
# Pagination drops the last page
