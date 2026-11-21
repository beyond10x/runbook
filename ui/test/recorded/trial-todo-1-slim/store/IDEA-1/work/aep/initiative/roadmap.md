---
format: aep.planning-md/3
id: initiative:roadmap
kind: initiative
status: draft
title: Roadmap
summary: 'epics: todolist'
revision: 1
---
## Goal

Give one person a personal to-do list that lives on a self-hosted Linux server. The user opens the web application and uses the list directly, with no account and no sign-in. The list stays available between browser sessions, and no other user can see or share it. The browser is only a view; the daemon's data file holds the authoritative items.

## Epics

### todolist: Anonymous single to-do list

- **Delivers:** A Rust daemon (standard library only) that serves a web UI and a JSON API for exactly one to-do list. Items are stored on the server in the daemon's data file. The app has no sign-in. Several instances can run side by side, each with its own port and data file.
- **Requirements covered:** r1 r2 r3
- **Done means:** The daemon starts on Linux and serves the web UI and JSON API. A user can add, change, and remove items with no sign-in. Items persist in the data file across browser sessions and daemon restarts. The browser keeps no authoritative copy. The app holds exactly one list and exposes no sharing or additional lists. Two instances started with different ports and data files run without interfering.

## Sequence

There is one epic, so there is no ordering between epics. Within the epic, the data file storage (r3) and the single list model (r1) come first, because the API and web UI depend on them. The anonymous access (r2) is a property of the whole surface and is verified last, once the UI and API exist.

## Out of scope

- No sign-in, accounts, or user identities (r2).
- No sharing: no other user can view or share the list (r1).
- No multiple lists: the application holds exactly one to-do list (r1).
- No browser-side source of truth: the browser does not keep the authoritative copy of the items (r3).
