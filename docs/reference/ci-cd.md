# CI/CD Selective Execution Guide

ONΛJI features native Git-aware selective execution. In pull requests and continuous integration environments, ONΛJI inspects git history to evaluate changed files, maps them to registered DAG resources, and computes the minimal set of affected tasks.

## Why Selective CI?

In typical monorepos or multi-service projects, full test suites can take 15–40 minutes. With `onaji affected`, if a developer only touches an OpenAPI contract or a single frontend component, ONΛJI executes **only the causal descendants** of those changes.

```
       [PR Changes]
            │ (git diff origin/main...HEAD)
            ▼
    [Matched Resources]
            │ (causal impact analysis)
            ▼
   [Only Affected Tasks] ────► [Execution in seconds]
```

---

## 1. Quick Usage

### Preview affected tasks
```bash
onaji affected --since origin/main
```

### Run only affected tasks in CI
```bash
onaji affected --since origin/main --run
```

### Run only affected tests and their prerequisites
```bash
onaji affected --since origin/main --tests --run
```

### Output machine-readable JSON for matrix builds
```bash
onaji affected --since origin/main --output json
```

---

## 2. GitHub Actions Workflow

Add this job to `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  pull_request:
    branches: [main]

jobs:
  selective-build:
    name: ONΛJI Selective CI
    runs-on: ubuntu-latest
    steps:
      - name: Checkout Code
        uses: actions/checkout@v4
        with:
          # fetch-depth: 0 is required for git diff against base branch
          fetch-depth: 0

      - name: Install ONΛJI
        run: curl -sSL https://raw.githubusercontent.com/odiador/onaji/main/install.sh | bash

      - name: Run Affected Tasks
        run: |
          onaji affected \
            --since "origin/${{ github.base_ref || 'main' }}" \
            --run \
            --output github
```

---

## 3. GitLab CI

```yaml
onaji-affected:
  stage: test
  image: ubuntu:latest
  before_script:
    - apt-get update && apt-get install -y git curl
    - curl -sSL https://raw.githubusercontent.com/odiador/onaji/main/install.sh | bash
    - git fetch origin $CI_MERGE_REQUEST_TARGET_BRANCH_NAME:origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME
  script:
    - onaji affected --since origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME --run
  only:
    - merge_requests
```
