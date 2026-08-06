#!/usr/bin/env python3
"""Generate new beat content (situations/choices/outcomes) for beats.json using
LangChain + Together AI, few-shotted on the existing authored set so new entries
match tone, tag vocabulary, and schema exactly.

Per CLAUDE.md: "LLMs at authoring time only" -- this is exactly that. Output is
NEVER written into beats.json directly; it goes to a separate draft file for
human review and manual merge, so a bad generation can't silently corrupt the
shipped content pool.

Usage:
  content-gen/.venv is not used (PEP 668 box; installed --user --break-system-packages).
  python3 content-gen/gen_beats.py --n-situations 5 --n-choices 8 --n-outcomes 10
  python3 content-gen/gen_beats.py --phase defend --n-situations 3 --n-choices 5 --n-outcomes 6
"""
from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Literal, Optional

from pydantic import BaseModel, Field
from langchain_openai import ChatOpenAI

ROOT = Path(__file__).parent.parent
BEATS_JSON = ROOT / "beats.json"
ENV_FILE = Path.home() / ".openclaw" / ".env"


def load_together_key() -> str:
    if not ENV_FILE.exists():
        sys.exit(f"no .env at {ENV_FILE}")
    for line in ENV_FILE.read_text().splitlines():
        if line.startswith("TOGETHER_API_KEY="):
            return line.split("=", 1)[1].strip().strip("'\"")
    sys.exit("TOGETHER_API_KEY not found in ~/.openclaw/.env")


PHASES = ("attack", "defend", "key", "setpiece", "positioning")
ATTRS = (
    "BallControl", "Tackling", "LongPassing", "ShortPassing", "Finishing",
    "Crossing", "Composure", "SprintSpeed", "Dribbling", "Vision", "Marking",
    "Positioning", "Heading", "Agility", "LongShots", "Strength", "Reactions",
)


class Situation(BaseModel):
    id: str = Field(description="snake_case, unique, descriptive")
    phase: Literal[PHASES]
    bias: list[int] = Field(min_length=3, max_length=3, description="3 ints, illustrative selector weight shape")
    setup: str = Field(description="1-2 sentences, second person, present tense, sets the scene")
    tags: list[str]


class Choice(BaseModel):
    id: str
    text: str = Field(description="short imperative phrase, the on-screen choice label")
    attr: Literal[ATTRS]
    difficulty: int = Field(ge=1, le=99)
    tags: list[str]


class Outcome(BaseModel):
    id: str
    text: str = Field(description="1 sentence, describes what happened, second person")
    output_delta: int = Field(description="signed; positive for success/prevented-danger, negative for failure")
    confidence: int = Field(description="signed headspace delta")
    frustration: int = Field(description="signed headspace delta")
    flow: int = Field(description="signed headspace delta")
    score_event: Optional[Literal["goal_for", "goal_against"]] = None
    stamina_cost: int = Field(ge=0)
    polarity: Literal["success", "failure"]


class BeatBatch(BaseModel):
    situations: list[Situation]
    choices: list[Choice]
    outcomes: list[Outcome]


def few_shot_examples(existing: dict, k: int = 4) -> str:
    import random
    sits = random.sample(existing["situations"], min(k, len(existing["situations"])))
    chs = random.sample(existing["choices"], min(k, len(existing["choices"])))
    outs = random.sample(existing["outcomes"], min(k, len(existing["outcomes"])))
    return json.dumps({"situations": sits, "choices": chs, "outcomes": outs}, indent=2)


def build_prompt(n_sit: int, n_ch: int, n_out: int, phase: Optional[str], existing: dict) -> str:
    existing_ids = {
        "situations": [s["id"] for s in existing["situations"]],
        "choices": [c["id"] for c in existing["choices"]],
        "outcomes": [o["id"] for o in existing["outcomes"]],
    }
    phase_note = f"\nAll new situations must use phase=\"{phase}\"." if phase else ""
    return f"""You are authoring content for a deterministic football career-sim's match
"beat" engine (Become the GOAT). Beats are short in-match dilemma-scenarios: a
situation sets up 2-4 choices, each choice resolves via a contest (player attribute
vs difficulty + RNG) into an outcome (success or failure branch).

Here are real examples already in the game, to match tone, scale, and tag vocabulary
(do NOT repeat these ids or reuse their exact text):

{few_shot_examples(existing)}

Existing IDs already taken (your new ids must NOT collide with any of these):
situations: {existing_ids['situations']}
choices: {existing_ids['choices']}
outcomes: {existing_ids['outcomes']}

Generate exactly {n_sit} new situations, {n_ch} new choices, and {n_out} new outcomes.{phase_note}
Keep prose terse and concrete (matching the examples' length and register — no
purple prose, no meta-commentary, second person present tense). Difficulty and
delta numbers should feel plausible next to the examples, not extreme outliers.
Vary phase/tags across the batch rather than repeating one situation type."""


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n-situations", type=int, default=5)
    ap.add_argument("--n-choices", type=int, default=8)
    ap.add_argument("--n-outcomes", type=int, default=10)
    ap.add_argument("--phase", choices=PHASES, default=None)
    ap.add_argument("--model", default="meta-llama/Llama-3.3-70B-Instruct-Turbo")
    ap.add_argument("--out", default=str(ROOT / "content-gen" / "beats_generated_draft.json"))
    args = ap.parse_args()

    existing = json.loads(BEATS_JSON.read_text())
    key = load_together_key()

    llm = ChatOpenAI(
        model=args.model,
        api_key=key,
        base_url="https://api.together.xyz/v1",
        temperature=0.9,
    ).with_structured_output(BeatBatch)

    prompt = build_prompt(args.n_situations, args.n_choices, args.n_outcomes, args.phase, existing)
    print(f"asking {args.model} via Together for {args.n_situations}/{args.n_choices}/{args.n_outcomes}...", file=sys.stderr)
    result: BeatBatch = llm.invoke(prompt)

    # Guard against id collisions the model might still produce despite instructions.
    existing_ids = {
        "situations": {s["id"] for s in existing["situations"]},
        "choices": {c["id"] for c in existing["choices"]},
        "outcomes": {o["id"] for o in existing["outcomes"]},
    }
    dropped = []
    situations = [s for s in result.situations if s.id not in existing_ids["situations"]]
    dropped += [s.id for s in result.situations if s.id in existing_ids["situations"]]
    choices = [c for c in result.choices if c.id not in existing_ids["choices"]]
    dropped += [c.id for c in result.choices if c.id in existing_ids["choices"]]
    outcomes = [o for o in result.outcomes if o.id not in existing_ids["outcomes"]]
    dropped += [o.id for o in result.outcomes if o.id in existing_ids["outcomes"]]

    out_path = Path(args.out)
    out_path.write_text(json.dumps({
        "situations": [s.model_dump() for s in situations],
        "choices": [c.model_dump() for c in choices],
        "outcomes": [o.model_dump() for o in outcomes],
    }, indent=2, ensure_ascii=False))

    print(f"wrote {len(situations)} situations, {len(choices)} choices, {len(outcomes)} outcomes -> {out_path}")
    if dropped:
        print(f"dropped {len(dropped)} id-collision(s), did not write: {dropped}", file=sys.stderr)
    print("REVIEW BEFORE MERGING into beats.json -- this is a draft, not authoritative content.")


if __name__ == "__main__":
    main()
