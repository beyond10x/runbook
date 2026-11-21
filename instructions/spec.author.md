Write the ESS (Executable System Specification) for the requirements into out/spec/.
in/example-spec/ is a complete specification that passes `ess specify validate --strict-requires`,
`ess specify compile` and `ess verify conform synthesize` with 0 refusals: copy its file layout,
keys and constructs exactly, and change only the content. Required:
- out/spec/system.yaml: format: ess/1, system: {{system}}, version: v1, a summary, and `domains`
  listing exactly the spec_domains used in in/requirements.yaml
- one file per domain under out/spec/domains/ (as in the example), with the entities, their
  states, the commands (with the refusals they can return) and the events the requirements need
- components.yaml and topology.yaml as in the example, naming your domains
- out/spec/traces.json (as in the example): {"traces": {"r1": [...], ...}}, for every
  requirement of in/requirements.yaml the full names of the declarations that deliver it. Only
  behaviour counts: a command, a view, or an actor whose `may` names commands. A domain, an
  entity, an event, an error, a type or an actor with no `may` delivers nothing, so a requirement
  traced only to those fails the trace check. Leave out only a requirement with `behaviour:
  constraint` or `behaviour: dropped`. If a requirement cannot be specified as behaviour, do not
  trace it to a domain or an actor: raise a finding about it (scope_changing: true)
Keep it as small as the requirements allow: no construct the requirements do not need. Never
write the text UNMAPPED: anywhere. If out/spec/ already holds a specification, fix it rather than
starting over.
