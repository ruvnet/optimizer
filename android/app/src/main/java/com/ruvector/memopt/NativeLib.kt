/**
 * RuVector Memory Optimizer - Native Library Bindings
 *
 * This Kotlin class provides the JNI bridge to the Rust native library.
 * All methods are thread-safe and can be called from any thread.
 *
 * IMPORTANT: Android does not allow direct memory manipulation from
 * userspace apps. This library provides READ-ONLY monitoring and
 * intelligent recommendations based on neural pattern analysis.
 *
 * Usage:
 * ```kotlin
 * // Get current memory status
 * val status = NativeLib.getMemoryStatus()
 *
 * // Get recommendations
 * val recommendations = NativeLib.getRecommendations()
 *
 * // Initialize neural engine for pattern learning
 * val modelPath = context.filesDir.absolutePath + "/neural"
 * NativeLib.initNeuralEngine(modelPath)
 * ```
 */
package com.ruvector.memopt

import android.util.Log
import org.json.JSONArray
import org.json.JSONObject

/**
 * Native library interface for RuVector Memory Optimizer.
 *
 * This singleton object loads the native Rust library and exposes
 * JNI functions for memory monitoring and neural analysis.
 */
object NativeLib {
    private const val TAG = "RuVectorNative"
    private var isLoaded = false

    init {
        try {
            System.loadLibrary("ruvector_memopt")
            isLoaded = true
            Log.i(TAG, "Native library loaded successfully")
        } catch (e: UnsatisfiedLinkError) {
            Log.e(TAG, "Failed to load native library: ${e.message}")
            isLoaded = false
        }
    }

    /**
     * Check if the native library is loaded.
     */
    fun isAvailable(): Boolean = isLoaded

    // =========================================================================
    // Native JNI Functions (implemented in Rust)
    // =========================================================================

    /**
     * Get current memory status as JSON string.
     *
     * @return JSON string with memory information:
     * ```json
     * {
     *   "total_physical_mb": 8192.0,
     *   "available_physical_mb": 4096.0,
     *   "memory_load_percent": 50,
     *   "used_physical_mb": 4096.0,
     *   "is_high_pressure": false,
     *   "is_critical": false
     * }
     * ```
     */
    external fun getMemoryStatus(): String

    /**
     * Get list of running processes as JSON array.
     *
     * @return JSON array of process objects:
     * ```json
     * [
     *   {"pid": 1234, "name": "com.example.app", "memory_mb": 256.5, "cpu_percent": 2.5},
     *   ...
     * ]
     * ```
     */
    external fun getProcessList(): String

    /**
     * Run memory analysis (read-only on Android).
     *
     * This performs analysis only - Android does not allow direct memory
     * manipulation from userspace apps without root.
     *
     * @return JSON string with analysis results
     */
    external fun runOptimization(): String

    /**
     * Get optimization recommendations based on current state.
     *
     * @return JSON string with recommendations:
     * ```json
     * {
     *   "should_optimize": true,
     *   "severity": "high",
     *   "confidence": 0.80,
     *   "reason": "Memory load at 85%",
     *   "top_memory_consumers": ["Chrome (512 MB)", ...],
     *   "suggested_actions": ["Close background apps", ...]
     * }
     * ```
     */
    external fun getRecommendations(): String

    /**
     * Initialize the neural decision engine.
     *
     * @param modelPath Path to store/load neural model data
     * @return true if initialization succeeded
     */
    external fun initNeuralEngine(modelPath: String): Boolean

    /**
     * Train the neural engine on an observed pattern.
     *
     * @param patternJson JSON string with pattern data:
     * ```json
     * {
     *   "load": 0.85,
     *   "success": true,
     *   "freed_mb": 256.0,
     *   "aggressive": false
     * }
     * ```
     * @return true if training succeeded
     */
    external fun trainPattern(patternJson: String): Boolean

    /**
     * Get the number of patterns learned by the neural engine.
     *
     * @return Pattern count, or -1 if engine not initialized
     */
    external fun getPatternCount(): Long

    /**
     * Get neural engine status as JSON.
     *
     * @return JSON string with status:
     * ```json
     * {
     *   "initialized": true,
     *   "pattern_count": 150,
     *   "ready": true
     * }
     * ```
     */
    external fun getNeuralStatus(): String

    // =========================================================================
    // Kotlin Helper Functions
    // =========================================================================

    /**
     * Parse memory status into a data class.
     */
    fun getMemoryStatusParsed(): MemoryStatus? {
        return try {
            val json = JSONObject(getMemoryStatus())
            MemoryStatus(
                totalPhysicalMb = json.getDouble("total_physical_mb"),
                availablePhysicalMb = json.getDouble("available_physical_mb"),
                memoryLoadPercent = json.getInt("memory_load_percent"),
                usedPhysicalMb = json.getDouble("used_physical_mb"),
                isHighPressure = json.getBoolean("is_high_pressure"),
                isCritical = json.getBoolean("is_critical")
            )
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse memory status: ${e.message}")
            null
        }
    }

    /**
     * Parse process list into a list of ProcessInfo objects.
     */
    fun getProcessListParsed(): List<ProcessInfo> {
        return try {
            val jsonArray = JSONArray(getProcessList())
            (0 until jsonArray.length()).map { i ->
                val obj = jsonArray.getJSONObject(i)
                ProcessInfo(
                    pid = obj.getInt("pid"),
                    name = obj.getString("name"),
                    memoryMb = obj.getDouble("memory_mb"),
                    cpuPercent = obj.getDouble("cpu_percent").toFloat()
                )
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse process list: ${e.message}")
            emptyList()
        }
    }

    /**
     * Parse recommendations into a data class.
     */
    fun getRecommendationsParsed(): Recommendation? {
        return try {
            val json = JSONObject(getRecommendations())
            val topConsumers = mutableListOf<String>()
            val consumersArray = json.getJSONArray("top_memory_consumers")
            for (i in 0 until consumersArray.length()) {
                topConsumers.add(consumersArray.getString(i))
            }

            val actions = mutableListOf<String>()
            val actionsArray = json.getJSONArray("suggested_actions")
            for (i in 0 until actionsArray.length()) {
                actions.add(actionsArray.getString(i))
            }

            Recommendation(
                shouldOptimize = json.getBoolean("should_optimize"),
                severity = json.getString("severity"),
                confidence = json.getDouble("confidence").toFloat(),
                reason = json.getString("reason"),
                topMemoryConsumers = topConsumers,
                suggestedActions = actions
            )
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse recommendations: ${e.message}")
            null
        }
    }

    /**
     * Parse neural status into a data class.
     */
    fun getNeuralStatusParsed(): NeuralStatus? {
        return try {
            val json = JSONObject(getNeuralStatus())
            NeuralStatus(
                initialized = json.getBoolean("initialized"),
                patternCount = json.getLong("pattern_count"),
                ready = json.getBoolean("ready")
            )
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse neural status: ${e.message}")
            null
        }
    }

    // =========================================================================
    // Data Classes
    // =========================================================================

    /**
     * Memory status information.
     */
    data class MemoryStatus(
        val totalPhysicalMb: Double,
        val availablePhysicalMb: Double,
        val memoryLoadPercent: Int,
        val usedPhysicalMb: Double,
        val isHighPressure: Boolean,
        val isCritical: Boolean
    ) {
        /**
         * Get available memory as a percentage.
         */
        val availablePercent: Int
            get() = (100 - memoryLoadPercent)

        /**
         * Get a human-readable status string.
         */
        val statusText: String
            get() = when {
                isCritical -> "Critical"
                isHighPressure -> "High Pressure"
                memoryLoadPercent > 60 -> "Moderate"
                else -> "Healthy"
            }
    }

    /**
     * Process information.
     */
    data class ProcessInfo(
        val pid: Int,
        val name: String,
        val memoryMb: Double,
        val cpuPercent: Float
    )

    /**
     * Optimization recommendation.
     */
    data class Recommendation(
        val shouldOptimize: Boolean,
        val severity: String,
        val confidence: Float,
        val reason: String,
        val topMemoryConsumers: List<String>,
        val suggestedActions: List<String>
    )

    /**
     * Neural engine status.
     */
    data class NeuralStatus(
        val initialized: Boolean,
        val patternCount: Long,
        val ready: Boolean
    )
}
