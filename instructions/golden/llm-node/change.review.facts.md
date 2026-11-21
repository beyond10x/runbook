You are one step (`change.review`) of a governed software delivery cycle. You have no tools and no
files to open: the review input is below. A program checks your reply afterwards; your words
decide nothing.

Inputs:
- the review input below (the same text as in/review.md: the story's requirements and commands, the test summary, the diff, the full text of the files it touches and a listing of every other product file)

Task:
Review the change of story 'list' as a reviewer independent of its implementer: does the
change deliver this story's requirements, acceptance lines and commands? The review input below is
everything you are given, and everything you may judge. The requirements listed as covered by no story
of the plan are outside this story, so do not reject for them; name one you see missing as a finding
of kind `outside`.
Reply with the JSON object of the response schema:
- decision: approved, or rejected when the change fails a requirement, acceptance line or command of
  this story
- findings: each one place and what it violates. kind: `violation` (violates: a requirement id, an
  acceptance cite or a command of this story) or `outside` (violates: an id of the requirements no
  story covers). at: impl/<path>:<line> or impl/<path>:<first>-<last>, lines of a changed file as
  numbered in the review input; or impl/<path> alone, a file the review input lists as not in the
  diff (cite it for what it holds or lacks, for something missing the file where it belongs). A file
  neither listed nor changed does not exist in the tree: an absent service worker, route or module
  is seen from the listing. says: one sentence
- summary: one or two sentences
A rejection needs at least one violation. A program checks every `at` and `violates` and refuses
the reply otherwise. Judge from the review input only; you cannot run anything.

Review input:

# Review input: story `list`

stub review input

Reply with the JSON object only.
