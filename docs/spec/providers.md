# Providers & model registry

- **Status:** draft (target design)
- **Scope:** how the engine reaches models — the Provider port and its wire protocol — the
  capability map, and the refreshable external index that supplies it.

Terms are in [`glossary.md`](glossary.md).

## The Provider port

A **Provider** is the model-facing adapter: given `{messages, toolset}` it returns the next
assistant turn — text and/or tool calls. The Station loop ([`engine.md`](engine.md)) drives
one Provider; it is the only model surface the loop knows. The Provider is distinct from the
**Driver**: the Driver sequences Run state Station-to-Station, the Provider faces the model.
The engine is provider-neutral — no model, vendor, or harness is named in the loop.

## Standard wire protocol

The default Provider speaks the **OpenAI-compatible Chat Completions** surface
(`POST /v1/chat/completions`, with `tools`, `tool_calls`, and `tool`-role results) — the wire
the widest set of engines implement. It is configured by **base URL + model id + key**, so any
compatible engine or gateway — OpenRouter, a LiteLLM proxy, vLLM, Ollama, or a native provider
that speaks it — is a configuration change, not a code change. A provider-specific Provider is
added behind the port only when a real provider needs its native protocol; none is built for an
imagined one. This retires the one-shot `claude -p` invocation in `llm.rs`.

## Capability map

Each known model carries a **capability record**: tool-call support, structured-output /
response-schema support, reasoning, input and output modalities, context limit, output limit,
and cost (input, output, cache). The map is the engine's model knowledge.

## Capability-checked binding

A Station declares the capabilities it requires of its model
([`blueprint.md`](blueprint.md)); the engine resolves a concrete model against the capability
map and refuses to bind one that lacks a required capability. A tool-calling Station therefore
cannot run on a model that cannot call tools — the mismatch is unrepresentable, not a runtime
surprise.

## Refreshable external index

The capability map is a **cached projection of an external catalog, never a list compiled into
the binary.** The primary source is **models.dev**; the index is fetched from a remote source
at runtime and cached in the persistence store ([`run-lifecycle.md`](run-lifecycle.md)). The
`piper models refresh` command re-fetches it, so new models become available **without
recompiling or redeploying**. A bundled snapshot ships with the binary as the offline floor:
Piper runs air-gapped, and a failed refresh leaves the last good index authoritative — stale,
never wrong. The catalog source sits behind an interface and is replaceable by configuration.
