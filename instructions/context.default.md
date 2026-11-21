Write the project context defaults to out/context.yaml with exactly these top-level keys:
audience, source_language, target_language, platform, kind, deployment, offline, licence. Each is a
mapping with exactly the keys value (a short string), status (always `default`) and reason (one
sentence). source_language and target_language are human languages (for example en) of the input
and of the product's text. kind is one of: service, multi-tenant application, CLI, library.
offline says whether and how the product works without a network connection: `no`, or `yes: `
and how (for example `yes: the browser client keeps a local copy and syncs when connected`).
Pick the simplest defaults the requirements allow.
