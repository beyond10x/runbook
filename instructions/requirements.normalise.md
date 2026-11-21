Normalise the idea's facts and wishes and the interview's answers into a requirements list.
Write out/requirements.yaml with one top-level key `requirements`, a list. Each requirement has
exactly these keys:
- id: r1, r2, ...
- text: one testable sentence: who does what, or what must hold
- cites: the ids it comes from: dissection items of type fact or wish, and question ids
  (q1, ...) whose answers it uses. Only ids that exist; at least one per requirement. An item of
  another type (belief, unknown, ambiguity, dark_spot) only beside the question that asks about it
- spec_domains: the specification domains it belongs to, each `<system>.<domain>` in lower case
  (for example todo.task). One system name for the whole list; 1 to 3 domains in total
Cite every fact, every wish and every answered question at least once. Keep the list short (at
most 8).
