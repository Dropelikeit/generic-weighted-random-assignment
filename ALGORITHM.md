# Algorithm Documentation

## Overview

The assignment engine generates **derangements** -- permutations where no element maps to itself -- using weighted random selection with historical penalty adjustments.

## Core Concepts

### Derangement

A derangement is a permutation `σ` of a set where `σ(i) ≠ i` for all `i`. In the context of assignments, this means no participant is assigned to themselves.

For `n` participants, the number of derangements is:

```
D(n) = n! × Σ(k=0 to n) (-1)^k / k!
```

This approaches `n! / e` for large `n`.

### Weighted Random Selection

For each giver, the algorithm constructs a probability distribution over all valid receivers. The probability of selecting receiver `j` for giver `i` is:

```
P(i → j) = w(i,j) / Σ(k ≠ i) w(i,k)
```

where `w(i,j)` is the adjusted weight for the pairing `(i, j)`.

## Penalty Strategies

### Linear Penalty (Default)

**Formula:**

```
adjusted_weight = base_weight / (1 + penalty_factor × historical_count)
```

**Properties:**
- Weight decreases proportionally with historical count.
- Never reaches zero (approaches 0 asymptotically).
- `penalty_factor = 0` disables penalties.
- `penalty_factor = 1` halves weight after 1 occurrence, thirds after 2, etc.

**Example:**

| Count | Factor=1.0 | Factor=2.0 | Factor=5.0 |
|---|---|---|---|
| 0 | 1.000 | 1.000 | 1.000 |
| 1 | 0.500 | 0.333 | 0.167 |
| 2 | 0.333 | 0.200 | 0.091 |
| 3 | 0.250 | 0.143 | 0.063 |
| 5 | 0.167 | 0.091 | 0.038 |

### Exponential Penalty

**Formula:**

```
adjusted_weight = base_weight × decay_rate^historical_count
```

**Properties:**
- Weight decays exponentially.
- Stronger penalty than linear for higher counts.
- `decay_rate = 0.5` halves weight per occurrence.
- `decay_rate` should be between 0.0 and 1.0.

**Example (decay_rate = 0.5):**

| Count | Weight |
|---|---|
| 0 | 1.000 |
| 1 | 0.500 |
| 2 | 0.250 |
| 3 | 0.125 |
| 5 | 0.031 |

### Threshold Penalty

**Formula:**

```
if historical_count >= threshold:
    adjusted_weight = base_weight × reduced_weight_factor
else:
    adjusted_weight = base_weight
```

**Properties:**
- No penalty below the threshold.
- Sharp reduction once threshold is reached.
- Good for "allow a few repeats, then strongly discourage."

## Assignment Algorithm

### Step-by-step Process

1. **Validate inputs** - At least 2 participants, no duplicates, valid history references.

2. **Build history map** - Convert history list to O(1) lookup table: `(giver, receiver) → count`.

3. **Shuffle participants** - Random ordering to avoid positional bias.

4. **Greedy weighted assignment** - For each participant in shuffled order:
   - a. Compute adjusted weights for all available receivers (excluding self).
   - b. Apply look-ahead to prevent dead-ends.
   - c. Select receiver using weighted random sampling.
   - d. Mark receiver as taken.

5. **Look-ahead logic** - Before selecting a receiver, check: "If I take this receiver, will all remaining unassigned givers still have at least one available receiver?" If not, exclude that candidate.

6. **Retry on failure** - If no valid assignment is found, start over with a new random shuffle (up to 100 attempts).

### Pseudocode

```
function generate(participants, history, strategy, seed):
    history_map = build_lookup(history)
    rng = seed ? seeded_rng(seed) : random_rng()

    for attempt in 1..MAX_RETRIES:
        order = shuffle(participants, rng)
        available = set(all participants)
        assignments = {}

        for giver in order:
            candidates = available - {giver}
            candidates = filter_lookahead(candidates, unassigned_givers)

            if candidates is empty:
                break  // dead-end, retry

            weights = [strategy.adjust(1.0, history_map[giver, r]) for r in candidates]
            receiver = weighted_random_select(candidates, weights, rng)

            assignments[giver] = receiver
            available.remove(receiver)

        if len(assignments) == len(participants):
            return assignments

    return error("failed after MAX_RETRIES")
```

### Complexity

- **Time**: O(n²) per attempt (n participants × n weight calculations).
- **Space**: O(n² + h) where h is the history size.
- **Expected attempts**: Typically 1-2 for most inputs. The look-ahead prevents most dead-ends.

### Determinism

When a seed is provided, the output is fully deterministic. The same seed, participants, history, and strategy will always produce the same assignments. This is useful for:

- Reproducible testing.
- Allowing users to "re-roll" by trying different seeds.
- Auditing and verification.
