Split the idea (in/idea.yaml, field `text`) into typed items. Write out/dissection.yaml with
one top-level key `items`, a list. Each item has exactly these keys:
- id: d1, d2, ...
- type: exactly one of fact, wish, belief, unknown, ambiguity, dark_spot
  (fact: stated as true; wish: a wanted outcome; belief: stated, unverified; unknown: a named gap;
  ambiguity: two or more readings; dark_spot: nothing said, but a builder needs it)
- span: a non-empty, exact, case-sensitive substring of the idea text, copied character by
  character (for a dark_spot, the nearest words it is about)
- note: one sentence: what is stated, or what is missing
At least one wish. At most 8 items.
