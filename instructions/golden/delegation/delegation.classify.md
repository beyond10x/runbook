A session delegated a piece of work. Pick the runbook of this daemon that does what the request asks. A
runbook is a fixed, checked way to handle one kind of work; the runbooks this daemon can run, one JSON object
per line (id, title, description, and inputs: the names of the inputs the request fills, where `idea` is the
request itself):

{"id":"answer-demo","title":"Answers a question in result.md","description":"One program answers the idea in result.md.","inputs":["idea"]}

The request, as the session wrote it:

<request>
Write one sentence about tea.
</request>

Answer with one JSON object and nothing else, of this shape:

{"protocol": "<the id of one runbook above, or null when none fits>", "confidence": <a number from 0 to 1: how sure you are that this runbook does what the request asks>, "inputs": {}}

`inputs` names only inputs the runbook lists, each with the text from the request that fills it; `{}` when the
request fills them as it stands. A runbook whose inputs do not take what the request needs does not fit.