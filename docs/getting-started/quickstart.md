# Quickstart (0 to 1 in 3 Minutes)

Get started with Onaji in any existing polyglot codebase.

## 1. Run Interactive Onboarding

```bash
cd my-polyglot-repo
onaji onboard
```

`onaji onboard` scans your repo, detects OpenAPI contracts, Cargo crates, Gradle builds, and TypeScript workspaces, bootstraps `onaji.yaml`, runs doctor diagnostics, and validates your DAG.

## 2. Inspect the Reactive DAG

```bash
onaji graph
```

## 3. Execute Incremental Pipeline

```bash
onaji run
```

Subsequent runs without changes will hit Tier 1/Tier 2 Cache instantly:

```bash
onaji run
# ✓ validate_openapi ............. (cached)
# ✓ generate_kotlin_client ....... (cached)
# ✓ compile_android .............. (cached)
```

## 4. Launch Real-Time Visualizer

```bash
onaji dashboard --open
```
