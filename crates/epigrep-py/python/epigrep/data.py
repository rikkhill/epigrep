"""Small demo datasets ("stories") for the visual harness and tests.

Each story bundles a tiny, hand-inspectable event stream with a recommended
pattern. The planted-noise story also carries exact ground-truth spans so the
harness can show precision/recall against known answers.
"""

from __future__ import annotations

import random
from dataclasses import dataclass, field

from ._core import Event, sort_events


@dataclass
class Story:
    key: str
    title: str
    description: str
    events: list
    pattern_text: str
    notes: str = ""
    # Ground-truth participating index lists (into ``events``), where known.
    expected: list | None = None
    default_exhaustive: bool = False
    extra_attrs: list = field(default_factory=list)


def care_pathway() -> Story:
    """A child enters care; a safeguarding flag follows within a window unless a
    placement change intervenes."""
    events = sort_events(
        [
            # child-1: a placement change sits between, so the absence guard blocks it.
            Event("child-1", 0, "entered_care"),
            Event("child-1", 2, "placement_change"),
            Event("child-1", 5, "safeguarding_flag", {"severity": 4}),
            # child-2: clean path within the window -> matches.
            Event("child-2", 0, "entered_care"),
            Event("child-2", 4, "safeguarding_flag", {"severity": 4}),
            # child-3: flag severity too low -> predicate filters it out.
            Event("child-3", 0, "entered_care"),
            Event("child-3", 3, "safeguarding_flag", {"severity": 2}),
        ]
    )
    return Story(
        key="care_pathway",
        title="Care pathway",
        description=(
            "entered_care followed by a high-severity safeguarding_flag within 5 "
            "time units, with no placement_change in between."
        ),
        events=events,
        pattern_text=(
            "entered_care -[<=5, no placement_change]-> "
            "safeguarding_flag[severity >= 3]"
        ),
        notes=(
            "child-1 is blocked by absence-between (placement_change at t=2); "
            "child-3 is filtered by the severity predicate; only child-2 matches."
        ),
    )


def observability_trace() -> Story:
    """A deploy is followed by an OOM kill on the same pod within a window."""
    events = sort_events(
        [
            Event("node-a", 0, "deploy", {"pod": "api"}),
            Event("node-a", 60, "heartbeat"),
            Event("node-a", 119, "oom_killed", {"pod": "api"}),
            # node-b: OOM is on a different pod than the deploy -> reference fails.
            Event("node-b", 0, "deploy", {"pod": "worker"}),
            Event("node-b", 100, "oom_killed", {"pod": "db"}),
        ]
    )
    return Story(
        key="observability_trace",
        title="Observability trace",
        description=(
            "A deploy followed within 120s by an oom_killed on the SAME pod, "
            "demonstrating capture/reference equality."
        ),
        events=events,
        pattern_text="deploy[pod as $p] -[<=120]-> oom_killed[pod == $p]",
        notes=(
            "node-a matches and captures pod=api. node-b does not: the OOM pod "
            "(db) differs from the captured deploy pod (worker)."
        ),
    )


def dead_end_story() -> Story:
    """First-successor commitment: an early candidate dead-ends and is not
    abandoned for a later one."""
    events = sort_events(
        [
            Event("p", 0, "A"),
            Event("p", 0, "B"),  # first B: no C within 1 -> dead-ends
            Event("p", 5, "C"),
            Event("p", 5, "B"),  # later B: a C is reachable within 1
            Event("p", 5, "C"),
        ]
    )
    return Story(
        key="dead_end",
        title="Dead-end (failed continuation)",
        description=(
            "A -> B -> C with a tight final window. The first B satisfies A->B "
            "but cannot reach a C in time."
        ),
        events=events,
        pattern_text="A -> B -[<=1]-> C",
        notes=(
            "FirstSuccessor commits to the first B (index 1) and then dead-ends; "
            "it does NOT backtrack to the later B to manufacture a match. Toggle "
            "exhaustive mode to see continuations that the committed search skips."
        ),
        expected=[],  # first-successor: no match
    )


def planted_noise(seed: int = 7) -> Story:
    """Random background noise with planted A->B episodes at known spans.

    Episodes are spaced well apart (>= 15 units) so each A's earliest in-window
    B is its own; noise is a distinct event type, so the A->B pattern can only
    match planted pairs. Ground truth is therefore independent of the matcher.
    """
    window = 3
    rng = random.Random(seed)
    raw = []
    expected_tags = []  # (partition, episode_id) of true positives

    for partition_index in range(2):
        partition = f"svc-{partition_index}"
        # Random background noise (a distinct type that the pattern never matches).
        for _ in range(8):
            raw.append(Event(partition, rng.randint(0, 60), "N"))
        # Planted positives: A then B within the window, well separated.
        for episode in range(3):
            base = episode * 18 + rng.randint(0, 4)
            gap = rng.randint(0, window)
            tag = f"{partition}-ep{episode}"
            raw.append(Event(partition, base, "A", {"episode": tag}))
            raw.append(Event(partition, base + gap, "B", {"episode": tag}))
            expected_tags.append(tag)
        # One planted negative: B falls outside the window.
        neg_base = 3 * 18 + 10
        raw.append(Event(partition, neg_base, "A", {"episode": f"{partition}-neg"}))
        raw.append(
            Event(partition, neg_base + window + 6, "B", {"episode": f"{partition}-neg"})
        )

    events = sort_events(raw)

    # Recover ground-truth spans by locating tagged events in the sorted stream.
    by_tag: dict[str, dict] = {}
    for index, event in enumerate(events):
        tag = event.attrs.get("episode")
        if tag is None:
            continue
        by_tag.setdefault(tag, {})[event.typ] = index
    expected = sorted(
        [by_tag[tag]["A"], by_tag[tag]["B"]] for tag in expected_tags
    )

    return Story(
        key="planted_noise",
        title="Planted noise (validation)",
        description=(
            "Random 'N' background with planted A->B episodes within a window. "
            "The matcher should recover exactly the planted spans."
        ),
        events=events,
        pattern_text="A -[<=3]-> B",
        notes=(
            "Background events use a type the pattern never matches, so any found "
            "span should be a planted positive. The out-of-window negatives must "
            "NOT appear."
        ),
        expected=expected,
    )


def all_stories() -> list:
    return [
        care_pathway(),
        observability_trace(),
        planted_noise(),
        dead_end_story(),
    ]


def story_by_key(key: str) -> Story:
    for story in all_stories():
        if story.key == key:
            return story
    raise KeyError(key)
