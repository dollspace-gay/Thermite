# Tone & voice

The standard for prose in this repo — documentation, design docs, and code
comments. The goal is an affirmative, precise case for the work. Credibility
comes from the content; the plainness is what makes it land.

## Affirmative, not defensive

Much of the existing prose was written in response to critique, so it argues
with an imagined skeptic. Remove that register.

- State what the system does and why. Build the case forward; don't preempt
  objections or rebut a hypothetical reader.
- Cut defensive scaffolding: "don't trust us," "a liar could type this," "the
  sharpest possible objection," "this is operational, not aspirational," "no new
  claims are made here," and similar. If a limitation is real, state it once as
  a plain fact and move on.
- One clear statement beats a repeated disclaimer. Say it once.

## Plain, not emphatic

- No ALL-CAPS for emphasis. No intensifiers used as emphasis — "exactly,"
  "precisely," "actually," "genuinely," "deliberately," "truly."
- No dramatic framing: "the single most," "the grind," "load-bearing" (when
  decorative), "hot enough to weld trust into software."
- Prefer the precise engineering claim to the vivid one. "Z3 returns a
  countermodel on failure" beats "every failure is a concrete counterexample,
  the property the whole ladder ranks by."

## Narrative stays localized

- A brief metaphor or narrative is welcome in introductions and conclusions —
  the README's opening, a doc's framing paragraph, a closing note.
- Everywhere else — mechanism descriptions, requirements, architecture, API
  docs, comments — stay technical.
- Shorthand terms are fine when defined nearby.

## Avoid the tics

These patterns read as performative and erode credibility. They are common in
AI-generated prose; cut them.

- **The antithesis pair.** "Not tested — proven." "Not as comments — as
  enforced syntax." "X, not Y." "Not just X but Y." State the positive claim
  directly: "proven for every input," "enforced as syntax."
- **Virtue adverbs.** "slides down *honestly*," "fails *loudly*," "*cleanly*
  separated." Describe the behavior; don't praise it. "A counterexample is a
  hard failure" needs no "honestly."
- **Em-dash drama.** A dash to land a punchy aside is a tell. Prefer a period.
  At most one dash per paragraph, and only for a genuine parenthetical.
- **Rhetorical bold.** Bold marks a defined term or a real label, not a
  sentence you want the reader to feel. Cut bold used for emphasis.
- **Cute asides.** "everyone goes home happy and wrong," "belt and suspenders,"
  "the kind of thing a liar could type." Delete them.

## Preserve substance

This is a register change, not a content change.

- Do not alter claims, numbers, identifiers, requirements, theorem names,
  certificate fields, or document structure.
- Do not soften a true, specific guarantee into a vague one. Calm the framing;
  keep the precision.
