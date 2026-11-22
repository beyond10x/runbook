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

This is everything you are given. Judge the change from it alone.

## Requirements this story delivers (a violation names the id)
- r1: A billing clerk creates, issues, pays and cancels invoices.

## Acceptance lines (a violation names the cite)
- aep/story/list.md:7: covers billing.invoice.CreateInvoice

## Commands it must serve (a violation names the command; the compiled specification)
- billing.invoice.CreateInvoice: {}

## Requirements no story of the plan covers (kind `outside`; never a reason to reject)
- r2: The customer is emailed when an invoice is created.

## Test summary (the conformance run that passed on this revision)
no counts found in the run's log
failing: none

## Change against the mainline tree (unified diff)
~~~diff
--- mainline/tests/last_page.rs
+++ impl/tests/last_page.rs
(new file: every line is added; its full text is below)
~~~

## Changed files, full text (the line numbers are the ones `at` cites; a file the diff does not touch is only listed at the end)

### impl/tests/last_page.rs (8 lines)
~~~
  1  use pager::{page, page_count};
  2  
  3  #[test]
  4  fn last_partial_page_is_shown() {
  5      let items: Vec<u32> = (1..=11).collect();
  6      assert_eq!(page_count(11, 5), 3);
  7      assert_eq!(page(&items, 3, 5), &[11]);
  8  }
~~~

## Product files not in the diff (path, size, and for source files the top-level items it declares; a file neither listed here nor in the diff does not exist in the tree; `at` may cite a listed file by path alone)
- Cargo.lock (149 bytes)
- Cargo.toml (105 bytes)
- README.md (333 bytes)
- Taskfile.yml (185 bytes)
- src/lib.rs (1252 bytes): page_count, page, tests

Reply with the JSON object only.
