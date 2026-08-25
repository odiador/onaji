package dev.odiador.onaji.daemon

import com.google.gson.Gson
import com.google.gson.JsonObject
import java.io.BufferedReader
import java.io.InputStreamReader
import java.io.PrintWriter
import java.net.Socket

data class DaemonStatus(val status: String, val version: String, val workersOnline: Int, val cacheHitRatio: Double)
data class TaskExplanation(val taskId: String, val rootCause: String, val triggerType: String, val cacheLookup: String, val durationMs: Long)

class OnajiDaemonClient(private val host: String = "127.0.0.1", private val port: Int = 9000) {
    private val gson = Gson()

    fun sendRpc(method: String, params: JsonObject = JsonObject()): JsonObject {
        return try {
            Socket(host, port).use { socket ->
                val writer = PrintWriter(socket.getOutputStream(), true)
                val reader = BufferedReader(InputStreamReader(socket.getInputStream()))

                val request = JsonObject().apply {
                    addProperty("jsonrpc", "2.0")
                    addProperty("id", 1)
                    addProperty("method", method)
                    add("params", params)
                }

                writer.println(request.toString())
                val responseLine = reader.readLine() ?: "{}"
                gson.fromJson(responseLine, JsonObject::class.java)
            }
        } catch (e: Exception) {
            JsonObject().apply { addProperty("error", e.message) }
        }
    }

    fun getStatus(): DaemonStatus {
        val resp = sendRpc("status")
        val result = resp.getAsJsonObject("result") ?: JsonObject()
        return DaemonStatus(
            status = result.get("status")?.asString ?: "offline",
            version = result.get("version")?.asString ?: "0.1.0",
            workersOnline = result.get("workers_online")?.asInt ?: 0,
            cacheHitRatio = result.get("cache_hit_ratio")?.asDouble ?: 0.0
        )
    }

    fun explainTask(taskId: String): TaskExplanation {
        val params = JsonObject().apply { addProperty("task", taskId) }
        val resp = sendRpc("task/explain", params)
        val result = resp.getAsJsonObject("result") ?: JsonObject()
        return TaskExplanation(
            taskId = result.get("task_id")?.asString ?: taskId,
            rootCause = result.get("root_cause")?.asString ?: "resource_modified",
            triggerType = result.get("trigger_type")?.asString ?: "fingerprint_diff",
            cacheLookup = result.get("cache_lookup")?.asString ?: "miss",
            durationMs = result.get("duration_ms")?.asLong ?: 0L
        )
    }
}
