/**
 * ALEC FreeRTOS Integration Example — EM500 Sensor on STM32 Cortex-M3
 *
 * Demonstrates calling alec_encode_multi() from a FreeRTOS sensor task on an
 * STM32F103 (or similar Cortex-M3) with 4.5 KB free RAM.
 *
 * Target: STM32 Cortex-M3, thumbv7m-none-eabi
 * RTOS:   FreeRTOS 10.x
 * RAM:    4.5 KB available for ALEC (heap + task stack)
 *
 * Build the Rust static library with:
 *   CARGO_PROFILE_RELEASE_OPT_LEVEL=z \
 *   CARGO_PROFILE_RELEASE_LTO=true \
 *   CARGO_PROFILE_RELEASE_PANIC=abort \
 *   CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
 *   cargo build -p alec-ffi --release --target thumbv7m-none-eabi \
 *       --no-default-features --features bare-metal
 *
 * Link against: libalec_ffi.a
 *
 * Memory budget (4.5 KB):
 *   ALEC heap (embedded-alloc):  3072 bytes  (reduced from default 8192)
 *   Task stack:                  1024 bytes
 *   Encode output buffer:         128 bytes  (on stack)
 *   Sensor value array:            40 bytes  (5 x f64, on stack)
 *   Remaining headroom:          ~256 bytes
 *
 * IMPORTANT: The default ALEC bare-metal heap is 8 KB. For 4.5 KB RAM you
 * must patch alec-ffi/src/lib.rs:bare_metal_support to reduce HEAP_SIZE to
 * 3072 or call alec_heap_init_with_size() if available. See "Heap sizing"
 * note below.
 */

#include <stdint.h>
#include <stddef.h>
#include "FreeRTOS.h"
#include "task.h"
#include "alec.h"

/* --------------------------------------------------------------------------
 * Hardware abstraction (replace with your BSP)
 * -------------------------------------------------------------------------- */

/** Read the EM500 5-in-1 sensor via I2C/UART. Returns 5 channels:
 *  [0] temperature (°C), [1] humidity (%), [2] barometric pressure (hPa),
 *  [3] light (lux), [4] CO2 (ppm). */
extern void em500_read_channels(double values[5]);

/** Transmit a buffer over LoRaWAN / UART / SPI to the gateway. */
extern void radio_transmit(const uint8_t *data, size_t len);

/** Get monotonic uptime in milliseconds (e.g. HAL_GetTick). */
extern uint32_t bsp_uptime_ms(void);

/* --------------------------------------------------------------------------
 * Configuration
 * -------------------------------------------------------------------------- */

#define NUM_CHANNELS       5
#define SAMPLE_INTERVAL_MS 10000   /* 10 s between readings            */
#define ENCODE_BUF_SIZE    128     /* Max encoded frame size (bytes)    */
#define SENSOR_TASK_STACK  256     /* Stack depth in words (1024 bytes) */
#define SENSOR_TASK_PRIO   (tskIDLE_PRIORITY + 2)

/* Source ID strings for per-channel context isolation */
static const char *channel_ids[NUM_CHANNELS] = {
    "temp", "humi", "baro", "light", "co2"
};

/* --------------------------------------------------------------------------
 * Sensor task
 * -------------------------------------------------------------------------- */

static void sensor_task(void *pvParameters)
{
    (void)pvParameters;

    /* --- Initialise ALEC heap (bare-metal allocator) --- */
    alec_heap_init();

    /* --- Create encoder (one-time allocation) ---
     * AlecEncoder is ~372 bytes of struct + dynamic BTreeMap allocations
     * inside the Context. With 5 channels the steady-state heap usage is
     * approximately 1.5-2.0 KB. */
    AlecEncoder *enc = alec_encoder_new();
    configASSERT(enc != NULL);

    double   values[NUM_CHANNELS];
    uint8_t  frame[ENCODE_BUF_SIZE];
    size_t   frame_len;

    for (;;)
    {
        /* 1. Sample all 5 channels */
        em500_read_channels(values);

        uint64_t ts = (uint64_t)bsp_uptime_ms();

        /* 2. Compress with adaptive multi-channel encoding.
         *    Each channel is independently classified (P1-P5) and
         *    encoded using the optimal strategy (Repeated, Delta8, etc.).
         *    P5 (disposable) channels are excluded from the frame. */
        AlecResult res = alec_encode_multi(
            enc,
            values,                 /* f64 array                     */
            NUM_CHANNELS,           /* 5 channels                    */
            &ts,                    /* shared timestamp              */
            channel_ids,            /* per-channel source IDs        */
            NULL,                   /* priorities: let classifier decide */
            frame,
            sizeof(frame),
            &frame_len
        );

        if (res == ALEC_OK && frame_len > 0)
        {
            /* 3. Transmit the compressed frame */
            radio_transmit(frame, frame_len);
        }

        /* 4. Sleep until next sample */
        vTaskDelay(pdMS_TO_TICKS(SAMPLE_INTERVAL_MS));
    }

    /* Unreachable — but clean up if the task were ever deleted */
    alec_encoder_free(enc);
}

/* --------------------------------------------------------------------------
 * Entry point
 * -------------------------------------------------------------------------- */

/**
 * Call from main() after BSP and FreeRTOS initialisation.
 *
 *   int main(void) {
 *       HAL_Init();
 *       SystemClock_Config();
 *       em500_init();
 *       radio_init();
 *       alec_sensor_start();
 *       vTaskStartScheduler();
 *       for (;;) {}
 *   }
 */
void alec_sensor_start(void)
{
    xTaskCreate(
        sensor_task,
        "ALEC",
        SENSOR_TASK_STACK,
        NULL,
        SENSOR_TASK_PRIO,
        NULL
    );
}

/* --------------------------------------------------------------------------
 * Heap sizing note
 * --------------------------------------------------------------------------
 *
 * The default bare-metal feature in alec-ffi allocates an 8 KB static heap.
 * On a 4.5 KB-constrained device, reduce this to 3 KB by editing
 * alec-ffi/src/lib.rs → bare_metal_support:
 *
 *     const HEAP_SIZE: usize = 3072;  // was 8192
 *     static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
 *
 * With 5 channels of slowly-varying sensor data (typical EM500 profile),
 * the Context will stabilise at ~1.5 KB of heap usage after ~50 samples.
 * The 3 KB heap leaves ~1.5 KB headroom for transient Vec allocations
 * during encoding.
 *
 * If you need all 5 channels AND checksums AND a larger dictionary,
 * consider a device with at least 8 KB free RAM (e.g. STM32L4).
 * -------------------------------------------------------------------------- */
