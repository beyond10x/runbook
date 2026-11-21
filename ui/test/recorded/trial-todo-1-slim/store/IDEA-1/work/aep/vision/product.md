---
format: aep.planning-md/3
id: vision:product
kind: vision
status: draft
title: Personal Anonymous To-Do List
summary: A small, self-hosted to-do list web application for one person who keeps a single list on the server, with no sign-in and no sharing.
revision: 1
---
# Personal Anonymous To-Do List

## Who it is for

A single end user who wants a personal to-do list for their own use. The user needs no account and no login.

## The problem

Keeping a to-do list should be simple. The user wants one list that stays available between browser sessions, without signing up for a service or managing accounts, and without the list being visible to anyone else.

## What it does

- Runs as a daemon on a Linux server and serves a web UI and a JSON API.
- Holds exactly one to-do list for the single user.
- Stores to-do items on the server, in the daemon's data file, so they remain available between sessions.
- Lets the user open the web application and use the list anonymously, with no sign-in.
- Treats the browser as a view only; it holds no copy that is the source of truth.
- Allows several instances to run side by side, each on its own port with its own data file.

## What it deliberately does not do

- No sign-in, accounts, or user identities (requirement r2).
- No sharing: no other user can view or share the list (requirement r1).
- No multiple lists: the application holds exactly one to-do list (requirement r1).
- No browser-side source of truth: the browser does not keep the authoritative copy of the items (requirement r3).
