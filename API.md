# API Documentation

## Base URL

```
http://localhost:8080
```

## Endpoints

### Health Check

```
GET /health
```

Returns the service health status and version.

**Response:**

```json
{
  "status": "ok",
  "version": "0.1.0"
}
```

---

### Generate Assignments

```
POST /assignments/generate
```

Generates weighted random assignments for the given participants.

**Request Body:**

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `participants` | `string[]` | Yes | - | List of participant identifiers (minimum 2) |
| `history` | `HistoricalPairing[]` | No | `[]` | Historical pairing records |
| `strategy` | `string` | No | `"linear"` | Penalty strategy: `"linear"`, `"exponential"`, or `"threshold"` |
| `penalty_factor` | `float` | No | `1.0` | Penalty factor for the linear strategy |
| `decay_rate` | `float` | No | `0.5` | Decay rate for the exponential strategy |
| `threshold` | `int` | No | `2` | Occurrence threshold for the threshold strategy |
| `reduced_weight_factor` | `float` | No | `0.1` | Weight multiplier when threshold is exceeded |
| `seed` | `int` | No | `null` | Optional seed for deterministic results |

**HistoricalPairing:**

| Field | Type | Description |
|---|---|---|
| `giver` | `string` | The giver participant ID (must exist in participants) |
| `receiver` | `string` | The receiver participant ID (must exist in participants) |
| `count` | `int` | Number of times this pairing has occurred |

**Example Request (Linear Strategy):**

```bash
curl -X POST http://localhost:8080/assignments/generate \
  -H "Content-Type: application/json" \
  -d '{
    "participants": ["Alice", "Bob", "Charlie", "Diana"],
    "history": [
      {"giver": "Alice", "receiver": "Bob", "count": 3},
      {"giver": "Bob", "receiver": "Charlie", "count": 1}
    ],
    "penalty_factor": 2.0,
    "seed": 42
  }'
```

**Example Request (Exponential Strategy):**

```bash
curl -X POST http://localhost:8080/assignments/generate \
  -H "Content-Type: application/json" \
  -d '{
    "participants": ["A", "B", "C", "D"],
    "strategy": "exponential",
    "decay_rate": 0.5,
    "history": [
      {"giver": "A", "receiver": "B", "count": 2}
    ]
  }'
```

**Example Request (Threshold Strategy):**

```bash
curl -X POST http://localhost:8080/assignments/generate \
  -H "Content-Type: application/json" \
  -d '{
    "participants": ["A", "B", "C", "D"],
    "strategy": "threshold",
    "threshold": 2,
    "reduced_weight_factor": 0.05
  }'
```

**Success Response (200):**

```json
{
  "assignments": [
    {"giver": "Alice", "receiver": "Charlie"},
    {"giver": "Bob", "receiver": "Diana"},
    {"giver": "Charlie", "receiver": "Bob"},
    {"giver": "Diana", "receiver": "Alice"}
  ]
}
```

**Error Response (400 - Bad Request):**

Returned for invalid strategy names.

```json
{
  "error": "unknown strategy: foo. Use 'linear', 'exponential', or 'threshold'"
}
```

**Error Response (422 - Unprocessable Entity):**

Returned for validation errors or algorithm failures.

```json
{
  "error": "validation error: need at least 2 participants"
}
```

---

## Integration Example (PHP)

```php
$response = file_get_contents('http://localhost:8080/assignments/generate', false,
    stream_context_create([
        'http' => [
            'method' => 'POST',
            'header' => 'Content-Type: application/json',
            'content' => json_encode([
                'participants' => ['Alice', 'Bob', 'Charlie', 'Diana'],
                'history' => [
                    ['giver' => 'Alice', 'receiver' => 'Bob', 'count' => 2]
                ],
                'penalty_factor' => 1.5,
            ]),
        ],
    ])
);

$result = json_decode($response, true);
foreach ($result['assignments'] as $assignment) {
    echo "{$assignment['giver']} → {$assignment['receiver']}\n";
}
```

## Integration Example (Python)

```python
import requests

response = requests.post("http://localhost:8080/assignments/generate", json={
    "participants": ["Alice", "Bob", "Charlie", "Diana"],
    "history": [
        {"giver": "Alice", "receiver": "Bob", "count": 2}
    ],
    "penalty_factor": 1.5,
})

for assignment in response.json()["assignments"]:
    print(f"{assignment['giver']} → {assignment['receiver']}")
```
