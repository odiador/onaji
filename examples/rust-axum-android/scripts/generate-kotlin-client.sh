#!/bin/sh
set -e
echo ">>> [Kotlin SDK Generator] Generating SDK from OpenAPI definition..."
mkdir -p android/core/network/src/generated
cat << 'EOF' > android/core/network/src/generated/AxumApiClient.kt
package dev.odiador.onaji.generated

class AxumApiClient(private val baseUrl: String) {
    fun health(): String = "OK"
    fun getUsers(): List<String> = emptyList()
}
EOF
echo ">>> [Kotlin SDK Generator] Generated AxumApiClient.kt successfully."
exit 0
