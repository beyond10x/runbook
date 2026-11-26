A session delegated a piece of work. Pick the runbook of this daemon that does what the request asks. A
runbook is a fixed, checked way to handle one kind of work; the runbooks this daemon can run, one JSON object
per line (id, title, description):

{{catalog}}

The request, as the session wrote it:

<request>
{{request}}
</request>

Answer with one JSON object and nothing else, of this shape:

{"protocol": "<the id of one runbook above, or null when none fits>", "confidence": <a number from 0 to 1: how sure you are that this runbook does what the request asks>, "inputs": {}}
