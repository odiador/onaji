# 13 — Onaji Distributed Scheduling & Work-Stealing Specification

## 1. Abstract
The Onaji Distributed Scheduler orchestrates task assignment across heterogeneous worker clusters under backpressure, preserving strict DAG dependency invariants while maximizing resource utilization through priority aging, worker slot accounting, and capability-aware work-stealing.

---

## 2. Priority Hierarchy & Invariants

```text
Critical   (Score: 100)
High       (Score: 75)
Normal     (Score: 50)
Low        (Score: 25)
Background (Score: 10)
```

**Core Invariant**: Task priority affects ONLY tasks in `Ready` state (all upstream DAG dependencies satisfied). Priority never promotes a blocked task before its prerequisites complete.

---

## 3. Aging & Fairness
To prevent starvation of `Low` priority tasks during bursts of `High` priority workloads:
$$\text{EffectivePriority} = \text{BasePriority} + \min\left(50, \left\lfloor \frac{\text{WaitTime (ms)}}{1000} \right\rfloor \times 5\right)$$

---

## 4. Backpressure & Slot Accounting
* Each worker announces `max_parallel_tasks` (slots).
* Total cluster capacity: $C = \sum_{w \in \text{Workers}} \text{available\_slots}(w)$.
* If $C = 0$, incoming tasks remain in `Queued` state (backpressure).
* Running tasks are tracked per worker. When a task completes, a slot opens and the scheduler instantly assigns the highest effective-priority compatible task.

---

## 5. Capability-Aware Work-Stealing
When worker $W_i$ becomes idle and its local queue is empty:
1. $W_i$ inspects queues of busy peer workers.
2. Identifies a candidate task $T$.
3. Checks invariant: $\text{requirements}(T) \subseteq \text{capabilities}(W_i)$.
4. Steals $T$ atomically and transitions state to `Assigned`.
