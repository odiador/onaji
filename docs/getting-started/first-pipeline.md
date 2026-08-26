# Writing Your First Pipeline (`onaji.yaml`)

Onaji models engineering workflows as a **Causal DAG** connecting `resources`, `artifacts`, and `tasks`.

```yaml
version: "1"
name: "polyglot-pipeline"

resources:
  openapi_spec:
    type: file
    path: "./api/openapi.yaml"
    debounce_ms: 200
    description: "REST API Specification"

artifacts:
  openapi_contract:
    description: "Validated REST schema"
  kotlin_client_sdk:
    description: "Generated Kotlin API bindings"
  android_binaries:
    description: "Compiled APK / AAR artifacts"

tasks:
  validate_openapi:
    inputs: [openapi_spec]
    outputs: [openapi_contract]
    run: "redocly lint ./api/openapi.yaml"

  generate_kotlin_client:
    inputs: [openapi_contract]
    outputs: [kotlin_client_sdk]
    run: "openapi-generator generate -i ./api/openapi.yaml -g kotlin -o ./android/client"

  compile_android:
    inputs: [kotlin_client_sdk]
    outputs: [android_binaries]
    run: "./gradlew assembleDebug"
    priority: high
```

## Validate and Run

```bash
onaji check
onaji run
```
