Decide whether to approve `{{action}}`: the {{subject}} at revision {{revision}} (in/{{path}}).
Approve when it is consistent with the idea, the requirements and the context, and every check in
in/checks.json about it holds. Reject when it contradicts a requirement or is not usable; say what is
wrong.
How to read the inputs: AEP artifacts are small by design. The roadmap (in/aep/initiative/roadmap.md)
names its epics in its summary; each epic is its own file in/aep/epic/<name>.md, and the roadmap is
the two together. The requirements cover the whole product; one epic or story need not restate them.
In in/checks.json a claim's `value` is the check's result at the current revision;
`excluded_evidence` only lists older records set aside, which is normal after a rewrite. The open
obligation of what you decide (for example roadmap.required) is open because your approval is
missing.
