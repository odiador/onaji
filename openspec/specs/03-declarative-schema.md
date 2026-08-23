# OpenSpec 03: Declarative Configuration Schema (`onaji.yaml`)

**Status**: Approved  
**Package**: `dev.odiador.onaji.config`  
**Domain**: Manifest Grammar, Artifacts & Tasks Declaration  

---

## 1. Syntax Specification

El manifiesto `onaji.yaml` describe el modelo de producción/consumo del proyecto:

```yaml
version: "1"
name: "cross-platform-sync"

# 1. Recursos primarios (Fuentes externas)
resources:
  openapi_spec:
    type: file
    path: "./backend/api/openapi.yaml"
    debounce_ms: 250

  backend_health:
    type: http
    url: "http://localhost:8080/health"
    interval_ms: 2000
    headers:
      Accept: "application/json"

# 2. Artefactos con identidad de grafo
artifacts:
  validated_contract:
    description: "Contrato OpenAPI validado y sin errores de sintaxis"
  kotlin_client_sdk:
    description: "Código fuente Kotlin generado en el módulo de red Android"
  android_build_classes:
    description: "Binarios compilados de Gradle"

# 3. Tareas (Nodos de transformación en el DAG)
tasks:
  validate_contract:
    inputs:
      - openapi_spec
    outputs:
      - validated_contract
    run: "npx @redocly/cli lint ./backend/api/openapi.yaml"

  generate_kotlin_client:
    inputs:
      - validated_contract
    outputs:
      - kotlin_client_sdk
    run: "./scripts/generate-kotlin-client.sh"

  compile_and_test_android:
    inputs:
      - kotlin_client_sdk
    outputs:
      - android_build_classes
    run: "./gradlew :androidApp:compileDebugKotlin :androidApp:testDebugUnitTest"
    cache:
      paths:
        - "androidApp/build.gradle.kts"
```

---

## 2. Inferencia Automática de Grafo

Onaji no requiere que el usuario declare `depends_on` manualmente si los `inputs` y `outputs` están declarados:
* `generate_kotlin_client` consume `validated_contract`, que es producido por `validate_contract`.
* Por lo tanto, `validate_contract` ➔ `generate_kotlin_client` se convierte automáticamente en una arista del DAG.
