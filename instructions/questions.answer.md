Answer the interview. out/questions.yaml is a copy of the current interview: edit it in place.
For every question with scope_changing: false, set `status: accepted` and `answer` to its default,
or `status: changed` and `answer` to a better answer when the default contradicts the idea, and
set `answered_by: non-interactive`. Leave every question with scope_changing: true exactly as it is,
whatever its status and answer (open, or already answered by a person): they are answered through
scope.answer, by the principal that holds that capability. Change nothing else.
