You describe how to drive one delivered software product through its HTTP JSON interface, so that a
program can hold it to the conformance suite of its specification. You change nothing; you only read.

In the current directory: `product/` is the delivered source tree (read its README, its routes and its
start command), `spec/` the specification it was built to, `ir.json` that specification compiled
(`commands` with `input` and `outcomes`, `events`, `errors`, `views`).

Reply with one route table (format sdc.conformance-table/1), exactly the reply schema:
- `build`: the argv that builds the product inside its tree, or null. Rust: ["cargo", "build", "--release", "--locked", "--offline"].
- `start.argv`: the argv that starts the built product, run in the tree. argv[0] is a path relative to the
tree (e.g. target/release/<bin>) or one of cargo, go, node, npm, deno, java, dotnet. Use the placeholders
{port} (required), {host} (127.0.0.1), {data_dir} (an empty directory), {data_file} (a file in it).
`start.from`: [{file, line}], the README or manifest (Cargo.toml, package.json, go.mod, pyproject.toml,
Procfile, Taskfile.yml, Makefile, deno.json, justfile) of the tree and the 1-based lines that state this
start command for normal use; together the cited lines must contain the program's file name and every
argument that holds no placeholder, except a value at the place of an operator value of a cited usage
line (`<partner-id>`: any value; `<a|b>`: one of them). Start the binary the build made, never `cargo run` or `go run`:
where the README says `cargo run --release -- <port> <file>`, start ["target/release/<bin>", "{port}",
"{data_file}"] and cite that README line and the Cargo.toml line naming the package or binary.
Never use an argument or a header the README describes as a test, demo or mock mode.
`start.args` (optional): where the README leaves a start value to the operator (e.g. <partner-account-id>)
and the specification has an input field for it, [{"name", "kind": "from_scenario", "field": <input field>,
"nth": 0, "when_absent": <the README's example value>}] and "{arg.<name>}" in start.argv: the adapter
starts the product per scenario with the nth distinct value the scenario sends in that field, else
when_absent (which must be on the cited lines). Never a script and never a write into the data file.
`start.ready`: a request the product answers once it is up, e.g. {"method": "GET", "path": "/"}.
- `captures` (optional): values the product prints when it starts, e.g. a link, a token or a board code
minted at start: {name, stream (stdout or stderr), regex (exactly one group), timeout_s (1-120)}.
"{capture.<name>}" reads one in paths, `headers`, bodies and templates. `bindings` (optional):
{input, capture} makes every input field of that name take the captured value instead of the
scenario's (e.g. the spec's link), and maps the captured value back in what the product answers.
Declare a capture only if the table uses it. If the product needs a sign-in, a token or a key,
send it in every request that needs it (a header or a body member, from a capture).
- `commands`: one entry per command of ir.json: `request` {method, path, body, headers}; in `path`,
`body` and `headers`, "{input.<field>}" is the command's input value. An optional input field
(type Optional<...>) is often left out by a scenario: do not name it in a path or query. `outcomes`:
one entry per outcome of the command,
`status` the HTTP status the product answers for it, `events` the events the outcome emits with a
`payload` naming every field of the event, `error` {error, fields} for an outcome that declares one.
In payloads and error fields "{response.<path>}" reads the response body, "{input.<field>}" the input,
"{lookup.<path>}" the body of a `lookup` request (a GET made after the response, e.g. for a state the
response lacks). An outcome that is external (`condition.kind` external) needs a `forced` entry: the
request on the command's own method and route that makes the product take it (e.g. a body member
or a query parameter that simulates a lost connection).
`status` may be a list when several statuses mean one outcome. Never list 404 on a wrong-state outcome:
the adapter itself reads a refusal (404, or the wrong-state status) of an id the scenario never created
as wrong-state, and 404 for an id the scenario created never. Two outcomes may share a status only with a `when_body` {path, equals} on each. The `error` name is the declared error,
written as is; payload fields and error `fields` are never literals: fill each from the response, a lookup, a capture or the input. A field filled from the
input is recorded unverified, because the check then compares the input with itself; prefer the
response where the product echoes the value.
- `views`: one entry per view: `request`, `rows` (the dotted path to the list of rows in the response
body, "" for the whole body), `fields` mapping each view field to its path in a row. Two views
cannot read the same request.
- Describe the product as it is: map each outcome to the statuses the product gives for the case the
outcome describes. Where a command, outcome or view has no route, give an entry with
only its name and `none` {kind, why} instead of inventing a route. kind
`not_reachable_over_the_interface`: the product does it, but no HTTP client can drive or read it
(it lives only in the browser, or only an operator at the console can trigger it); its scenarios
are counted not measurable. kind `not_implemented`: the product does not do it; its scenarios fail.
Say in `why` where you looked.
- `notes`: a few sentences on what you read and anything uncertain.
