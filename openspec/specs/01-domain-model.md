# OpenSpec 01: Domain Model & Semantic DAG

**Status**: Approved  
**Package**: `dev.odiador.onaji.core.domain`  
**Domain**: Entities, Invariants, Graph Relations, Impact Analysis  

---

## 1. Domain Entities & Taxonomy

Onaji modela el espacio de trabajo no como una lista ciega de watchers, sino como un **Grafo Semántico de Producción y Consumo**:

```
[ Resource ] ──(emits)──► [ Event ] ──(triggers)──► [ Task ]
                                                       │
                                              (consumes / produces)
                                                       ▼
                                                 [ Artifact ]
```

### 1.1 `Resource` (Fuente observable de verdad)
Representa una entidad externa o de origen que existe en el entorno y emite señales de cambio.
* **Tipos**: `File`, `Directory`, `HttpEndpoint`, `Process`, `GitRepository`.
* **Invariante**: Un Resource no es producido por una Task dentro del mismo ciclo reactivo; es una raíz o entrada primaria.

### 1.2 `Event` (Señal de mutación temporal)
Representa un cambio de estado en un Resource.
* **Atributos**: `id`, `resource_id`, `type` (`created`, `modified`, `deleted`, `available`, `degraded`), `timestamp`, `payload_hash`.
* **Regla**: Eventos ruidosos (ej. touch sin cambio de contenido o HTTP 304/5xx) son descartados antes de tocar el DAG.

### 1.3 `Artifact` (Dato / Objeto producido o consumido)
Representa una entidad con identidad y huella digital (fingerprint) producida por una Task o asociada a un Resource.
* **Ejemplos**: `OpenApiContract`, `KotlinClientSdk`, `GradleCompiledClasses`, `CoverageReport`.
* **Propiedades**: `name`, `fingerprint` (xxHash3), `location` (paths, memory), `metadata`.

### 1.4 `Task` (Unidad atómica de ejecución)
Un proceso o comando que transforma Artifacts de entrada en Artifacts de salida.
* **Declaración**:
  * `inputs`: Conjunto de Resources o Artifacts requeridos.
  * `outputs`: Conjunto de Artifacts que garantiza producir al finalizar con éxito.
  * `action`: Ejecución de comando (`command`), script o adapter interno.
* **Invariante**: Si los fingerprints de todos sus `inputs` son idénticos a la última ejecución exitosa, la Task se salta (`CACHE_HIT`).

### 1.5 `Pipeline` & `Graph` (Grafo Acíclico Dirigido)
El conjunto de Tasks interconectadas por relaciones de `produces` y `consumes`.

---

## 2. Impact Analysis Algorithm

Cuando un `Event` ocurre sobre un `Resource`:

1. **Root Identification**: Se localizan todas las `Task` que declaran dicho `Resource` (o su `Artifact` asociado) como `input`.
2. **Reachable Subgraph**: Se calcula la clausura transitiva hacia adelante (hijos downstream) únicamente de los nodos afectados.
3. **Execution Plan**:
   - Se ordenan topológicamente los nodos afectados.
   - Nodos sin dependencias mutuas se planifican en paralelo vía `tokio::spawn`.
   - Si una Task upstream falla, toda su rama downstream se cancela/aborta de forma determinista.
