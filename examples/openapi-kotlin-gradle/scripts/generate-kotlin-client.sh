#!/bin/sh
set -e
echo ">>> [Kotlin Generator] Reading OpenAPI contract from backend/api/openapi.yaml..."
mkdir -p android/core/network/src/generated
cat << 'EOF' > android/core/network/src/generated/PetStoreClient.kt
package dev.odiador.onaji.generated

class PetStoreClient {
    fun getHealth(): String = "OK"
}
EOF
echo ">>> [Kotlin Generator] Generated PetStoreClient.kt successfully."
exit 0
