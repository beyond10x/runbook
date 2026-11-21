---
format: aep.planning-md/3
id: story:storage
kind: story
status: active
title: Persist to-do items server-side
summary: 'Keep to-do items on the server in the daemon''s data file so they remain available between sessions, then: covers todo.storage.AddItem'
relations:
- delivers: epic:todolist
- serves: vision:product
scope:
- confidence: cited
  path: impl
revision: 3
transitions:
- {from: "draft", to: "active", at: "2026-11-20T23:57:52Z", actor: "human:operator", revision: 3}
---
# Persist to-do items server-side
