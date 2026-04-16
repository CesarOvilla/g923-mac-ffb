// Plugin SCS all-in-one para G923 Xbox en macOS.
//
// Corre DENTRO del proceso de ATS/ETS2. Recibe telemetría via
// callbacks del SDK y envía FFB al volante via IOHIDDeviceSetReport.
// No necesita daemon externo — evita el conflicto de multi-proceso
// que macOS impone sobre IOHIDManager.
//
// Compilar:
//   clang -arch x86_64 -shared -fvisibility=hidden -O2 \
//     -framework IOKit -framework CoreFoundation \
//     -o g923_telemetry.dylib g923_telemetry.c
//
// Instalar: copiar .dylib a ATS.app/Contents/MacOS/plugins/

#include <stdint.h>
#include <string.h>
#include <stdio.h>
#include <math.h>
#include <pthread.h>
#include <sys/mman.h>
#include <fcntl.h>
#include <unistd.h>
#include <IOKit/hid/IOHIDManager.h>

// ════════════════════════════════════════════════════════════════════
// G923 HID++ constants
// ════════════════════════════════════════════════════════════════════

#define G923_VID            0x046d
#define G923_PID            0xc26e
#define HIDPP_USAGE_PAGE    0xFF43
#define HIDPP_LONG_USAGE    0x0602

#define HIDPP_DEV_IDX       0xFF
#define HIDPP_SW_ID         0x01
#define HIDPP_REPORT_LONG   0x11
#define HIDPP_REPORT_VLONG  0x12

// Feature 0x8123 ForceFeedback — index 11 en firmware 0x3901
#define FFB_FEATURE_IDX     11

#define FFB_FN_RESET_ALL    1
#define FFB_FN_DOWNLOAD     2
#define FFB_FN_DESTROY      4

#define FFB_EFFECT_CONSTANT 0x00
#define FFB_EFFECT_SPRING   0x06
#define FFB_AUTOSTART       0x80

// ════════════════════════════════════════════════════════════════════
// Shared memory struct (para el monitor externo, opcional)
// ════════════════════════════════════════════════════════════════════

#define G923_SHM_NAME    "/g923_telemetry"
#define G923_MAGIC       0x47393233

typedef struct {
    uint32_t magic;
    uint32_t version;
    uint64_t frame;
    float    speed, rpm, steering, throttle, brake, clutch;
    float    accel_x, accel_y, accel_z;
    float    susp_deflection[4];
    uint8_t  on_ground[4];
    float    cargo_mass;
    uint8_t  paused;
    uint8_t  _pad[3];
} g923_telemetry_t;

// ════════════════════════════════════════════════════════════════════
// SCS SDK types (minimal, x64 macOS layout)
// ════════════════════════════════════════════════════════════════════

typedef int32_t   scs_result_t;
typedef uint32_t  scs_u32_t;
typedef const char *scs_string_t;
typedef void      *scs_context_t;

#define SCS_RESULT_ok             0
#define SCS_RESULT_unsupported   -1
#define SCS_RESULT_generic_error -7
#define SCS_U32_NIL              ((scs_u32_t)-1)
#define SCS_TELEMETRY_VERSION_1_01 ((1 << 16) | 1)
#define SCS_VALUE_TYPE_bool     1
#define SCS_VALUE_TYPE_float    5
#define SCS_VALUE_TYPE_fvector  7
#define SCS_TELEMETRY_CHANNEL_FLAG_each_frame 2
#define SCS_TELEMETRY_EVENT_frame_end      2
#define SCS_TELEMETRY_EVENT_paused         3
#define SCS_TELEMETRY_EVENT_started        4
#define SCS_TELEMETRY_EVENT_configuration  5

typedef struct { float x, y, z; } scs_value_fvector_t;
typedef struct {
    scs_u32_t type;
    scs_u32_t _pad;
    union {
        uint8_t              value_bool;
        scs_u32_t            value_u32;
        float                value_float;
        scs_value_fvector_t  value_fvector;
        uint8_t              _largest[40];
    };
} scs_value_t;

typedef void (*scs_telemetry_channel_callback_t)(
    scs_string_t, scs_u32_t, const scs_value_t *, scs_context_t);
typedef void (*scs_telemetry_event_callback_t)(
    scs_u32_t, const void *, scs_context_t);
typedef scs_result_t (*scs_telemetry_register_for_channel_t)(
    scs_string_t, scs_u32_t, scs_u32_t, scs_u32_t,
    scs_telemetry_channel_callback_t, scs_context_t);
typedef scs_result_t (*scs_telemetry_register_for_event_t)(
    scs_u32_t, scs_telemetry_event_callback_t, scs_context_t);
typedef void (*scs_log_t)(int32_t, scs_string_t);

typedef struct {
    scs_string_t game_name;
    scs_string_t game_id;
    scs_u32_t    game_version;
    scs_u32_t    _pad;
    scs_log_t    log;
} scs_common_t;

typedef struct {
    scs_common_t common;
    scs_telemetry_register_for_event_t      register_for_event;
    void *unregister_from_event;
    scs_telemetry_register_for_channel_t    register_for_channel;
    void *unregister_from_channel;
} scs_telemetry_init_params_t;

// ════════════════════════════════════════════════════════════════════
// Global state
// ════════════════════════════════════════════════════════════════════

static g923_telemetry_t  g_telem;
static g923_telemetry_t *g_shm = NULL;
static scs_log_t         g_log = NULL;
static IOHIDDeviceRef    g_hid_dev = NULL;
static IOHIDManagerRef   g_hid_mgr = NULL;

// FFB state
static int      g_ffb_ready = 0;
static uint32_t g_ffb_frame = 0;        // counter para cleanup periódico
static float    g_last_spring_coeff = 0;
static float    g_last_lateral = 0;

// FFB tuning
#define SPRING_BASE       3000.0f
#define SPRING_PER_KMH    250.0f
#define SPRING_MAX        30000.0f
#define SPRING_THRESHOLD  2000.0f

#define LATERAL_GAIN      5000.0f
#define LATERAL_MAX       20000.0f
#define LATERAL_THRESHOLD 800.0f
#define LATERAL_DEADZONE  500.0f

#define FFB_UPDATE_INTERVAL 4  // actualizar cada N frames (~4 frames a 60fps = ~15Hz)
#define FFB_CLEANUP_INTERVAL 300  // ResetAll cada N updates (~20 segundos)

static void log_msg(const char *msg) {
    if (g_log) g_log(0, msg);
}

// ════════════════════════════════════════════════════════════════════
// IOKit HID — encontrar y abrir el G923 HID++ collection
// ════════════════════════════════════════════════════════════════════

// Thread dedicado para HID I/O — replica la arquitectura de hidapi.
// macOS IOHIDManager necesita un run loop activo para que los reports
// realmente se entreguen al hardware via el endpoint USB.

static pthread_t       g_hid_thread;
static CFRunLoopRef    g_hid_runloop = NULL;
static volatile int    g_hid_thread_ready = 0;

static void hid_input_callback(void *context, IOReturn result,
                                void *sender, IOHIDReportType type,
                                uint32_t reportID, uint8_t *report,
                                CFIndex reportLength) {
    (void)context; (void)result; (void)sender;
    (void)type; (void)reportID; (void)report; (void)reportLength;
}

static void set_int_prop(CFMutableDictionaryRef dict, const char *key, int val) {
    CFStringRef k = CFStringCreateWithCString(NULL, key, kCFStringEncodingUTF8);
    CFNumberRef v = CFNumberCreate(NULL, kCFNumberIntType, &val);
    CFDictionarySetValue(dict, k, v);
    CFRelease(k);
    CFRelease(v);
}

static void *hid_thread_func(void *arg) {
    (void)arg;

    g_hid_mgr = IOHIDManagerCreate(kCFAllocatorDefault, kIOHIDOptionsTypeNone);
    if (!g_hid_mgr) { g_hid_thread_ready = -1; return NULL; }

    // NULL matching: descubrir TODOS los HID devices del sistema.
    // Filtrar por VID/PID en software para encontrar colecciones vendor
    // que no aparecen con matching selectivo.
    IOHIDManagerSetDeviceMatching(g_hid_mgr, NULL);

    g_hid_runloop = CFRunLoopGetCurrent();
    IOHIDManagerScheduleWithRunLoop(g_hid_mgr, g_hid_runloop, kCFRunLoopDefaultMode);
    IOHIDManagerOpen(g_hid_mgr, kIOHIDOptionsTypeNone);

    // Correr el run loop 1 segundo para descubrimiento asíncrono de devices
    CFRunLoopRunInMode(kCFRunLoopDefaultMode, 1.0, false);

    CFSetRef devices = IOHIDManagerCopyDevices(g_hid_mgr);
    if (!devices || CFSetGetCount(devices) == 0) {
        if (devices) CFRelease(devices);
        if (g_log) g_log(0, "[g923] No se encontraron HID devices");
        g_hid_thread_ready = -1;
        return NULL;
    }

    CFIndex count = CFSetGetCount(devices);
    IOHIDDeviceRef *devs = (IOHIDDeviceRef *)malloc(count * sizeof(IOHIDDeviceRef));
    CFSetGetValues(devices, (const void **)devs);

    {
        char msg[64];
        snprintf(msg, sizeof(msg), "[g923] Total HID devices: %d", (int)count);
        if (g_log) g_log(0, msg);
    }

    g_hid_dev = NULL;
    IOHIDDeviceRef joystick_dev = NULL;
    int g923_count = 0;

    for (CFIndex i = 0; i < count; i++) {
        CFNumberRef vid_ref = IOHIDDeviceGetProperty(devs[i], CFSTR(kIOHIDVendorIDKey));
        CFNumberRef pid_ref = IOHIDDeviceGetProperty(devs[i], CFSTR(kIOHIDProductIDKey));
        int vid = 0, pid = 0;
        if (vid_ref) CFNumberGetValue(vid_ref, kCFNumberIntType, &vid);
        if (pid_ref) CFNumberGetValue(pid_ref, kCFNumberIntType, &pid);
        if (vid != G923_VID || pid != G923_PID) continue;

        g923_count++;
        CFNumberRef up_ref = IOHIDDeviceGetProperty(devs[i], CFSTR(kIOHIDPrimaryUsagePageKey));
        CFNumberRef u_ref = IOHIDDeviceGetProperty(devs[i], CFSTR(kIOHIDPrimaryUsageKey));
        int up_val = 0, u_val = 0;
        if (up_ref) CFNumberGetValue(up_ref, kCFNumberIntType, &up_val);
        if (u_ref) CFNumberGetValue(u_ref, kCFNumberIntType, &u_val);

        char msg[128];
        snprintf(msg, sizeof(msg), "[g923] G923 #%d: usage_page=0x%04x usage=0x%04x",
                 g923_count, up_val, u_val);
        if (g_log) g_log(0, msg);

        if (up_val == HIDPP_USAGE_PAGE && u_val == HIDPP_LONG_USAGE) {
            g_hid_dev = devs[i];
            CFRetain(g_hid_dev);
            if (g_log) g_log(0, "[g923] → ¡¡¡ ENCONTRADA colección HID++ (0xFF43/0x0602) !!!");
        }
        if (up_val == 0x0001 && u_val == 0x0004) {
            joystick_dev = devs[i];
        }
    }

    if (!g_hid_dev && joystick_dev) {
        g_hid_dev = joystick_dev;
        CFRetain(g_hid_dev);
        if (g_log) g_log(0, "[g923] → HID++ NO encontrada, fallback a Joystick");
    }

    {
        char msg[64];
        snprintf(msg, sizeof(msg), "[g923] G923 colecciones encontradas: %d", g923_count);
        if (g_log) g_log(0, msg);
    }

    free(devs);
    CFRelease(devices);

    if (!g_hid_dev) {
        if (g_log) g_log(0, "[g923] G923 no encontrado");
        g_hid_thread_ready = -1;
        return NULL;
    }

    IOReturn ret = IOHIDDeviceOpen(g_hid_dev, kIOHIDOptionsTypeNone);
    if (ret != kIOReturnSuccess) {
        CFRelease(g_hid_dev);
        g_hid_dev = NULL;
        g_hid_thread_ready = -1;
        return NULL;
    }

    IOHIDDeviceScheduleWithRunLoop(g_hid_dev, g_hid_runloop, kCFRunLoopDefaultMode);

    static uint8_t input_buf[64];
    IOHIDDeviceRegisterInputReportCallback(g_hid_dev,
        input_buf, sizeof(input_buf), hid_input_callback, NULL);

    // Log del device que encontramos para diagnóstico
    {
        char msg[256];
        CFNumberRef up_ref = IOHIDDeviceGetProperty(g_hid_dev, CFSTR(kIOHIDPrimaryUsagePageKey));
        CFNumberRef u_ref = IOHIDDeviceGetProperty(g_hid_dev, CFSTR(kIOHIDPrimaryUsageKey));
        CFStringRef prod = IOHIDDeviceGetProperty(g_hid_dev, CFSTR(kIOHIDProductKey));
        int up_val = 0, u_val = 0;
        if (up_ref) CFNumberGetValue(up_ref, kCFNumberIntType, &up_val);
        if (u_ref) CFNumberGetValue(u_ref, kCFNumberIntType, &u_val);
        char prod_str[64] = "(?)";
        if (prod) CFStringGetCString(prod, prod_str, sizeof(prod_str), kCFStringEncodingUTF8);
        snprintf(msg, sizeof(msg),
            "[g923] Device abierto: product='%s' usage_page=0x%04x usage=0x%04x",
            prod_str, up_val, u_val);
        log_msg(msg);
    }

    // Señalar que estamos listos
    g_hid_thread_ready = 1;

    // Correr el run loop para siempre (procesa callbacks HID)
    CFRunLoopRun();

    return NULL;
}

static int hid_open(void) {
    g_hid_thread_ready = 0;
    if (pthread_create(&g_hid_thread, NULL, hid_thread_func, NULL) != 0) {
        return 0;
    }

    // Esperar hasta que el thread abra el device (máx 3s)
    for (int i = 0; i < 60 && g_hid_thread_ready == 0; i++) {
        usleep(50000); // 50ms
    }

    if (g_hid_thread_ready != 1) {
        log_msg("[g923] HID thread falló al abrir el device");
        return 0;
    }

    log_msg("[g923] HID++ device abierto en thread dedicado");
    return 1;
}

static void hid_close(void) {
    // Marcar que ya no estamos listos para evitar escrituras tardías
    g_ffb_ready = 0;

    if (g_hid_runloop) {
        CFRunLoopRef rl = g_hid_runloop;
        g_hid_runloop = NULL;
        CFRunLoopStop(rl);
        pthread_join(g_hid_thread, NULL);
    }
    if (g_hid_dev) {
        IOHIDDeviceClose(g_hid_dev, kIOHIDOptionsTypeNone);
        CFRelease(g_hid_dev);
        g_hid_dev = NULL;
    }
    if (g_hid_mgr) {
        IOHIDManagerClose(g_hid_mgr, kIOHIDOptionsTypeNone);
        CFRelease(g_hid_mgr);
        g_hid_mgr = NULL;
    }
}

// ════════════════════════════════════════════════════════════════════
// HID++ command helpers (fire-and-forget via IOHIDDeviceSetReport)
// ════════════════════════════════════════════════════════════════════

static int g_send_count = 0;

static void hidpp_send_long(uint8_t feature_idx, uint8_t function,
                            const uint8_t *params, int param_len) {
    if (!g_hid_dev) return;
    uint8_t data[19];
    memset(data, 0, sizeof(data));
    data[0] = HIDPP_DEV_IDX;
    data[1] = feature_idx;
    data[2] = (function << 4) | HIDPP_SW_ID;
    if (params && param_len > 0 && param_len <= 16)
        memcpy(data + 3, params, param_len);
    // Intentar como output report primero, luego feature report como fallback
    IOReturn ret = IOHIDDeviceSetReport(g_hid_dev, kIOHIDReportTypeOutput,
                         HIDPP_REPORT_LONG, data, sizeof(data));
    if (ret != kIOReturnSuccess) {
        ret = IOHIDDeviceSetReport(g_hid_dev, kIOHIDReportTypeFeature,
                         HIDPP_REPORT_LONG, data, sizeof(data));
    }
    g_send_count++;
    if (g_send_count <= 5) {
        char msg[128];
        snprintf(msg, sizeof(msg),
            "[g923] send_long #%d: fn=%d ret=0x%x data=%02x %02x %02x %02x %02x",
            g_send_count, function, ret, data[0], data[1], data[2], data[3], data[4]);
        log_msg(msg);
    }
}

static void hidpp_send_vlong(uint8_t feature_idx, uint8_t function,
                             const uint8_t *params, int param_len) {
    if (!g_hid_dev) return;
    uint8_t data[63];
    memset(data, 0, sizeof(data));
    data[0] = HIDPP_DEV_IDX;
    data[1] = feature_idx;
    data[2] = (function << 4) | HIDPP_SW_ID;
    if (params && param_len > 0 && param_len <= 60)
        memcpy(data + 3, params, param_len);
    IOReturn ret = IOHIDDeviceSetReport(g_hid_dev, kIOHIDReportTypeOutput,
                         HIDPP_REPORT_VLONG, data, sizeof(data));
    if (ret != kIOReturnSuccess) {
        ret = IOHIDDeviceSetReport(g_hid_dev, kIOHIDReportTypeFeature,
                         HIDPP_REPORT_VLONG, data, sizeof(data));
    }
    g_send_count++;
    if (g_send_count <= 5) {
        char msg[128];
        snprintf(msg, sizeof(msg),
            "[g923] send_vlong #%d: fn=%d ret=0x%x data=%02x %02x %02x",
            g_send_count, function, ret, data[0], data[1], data[2]);
        log_msg(msg);
    }
}

// ════════════════════════════════════════════════════════════════════
// FFB commands
// ════════════════════════════════════════════════════════════════════

static void ffb_reset_all(void) {
    hidpp_send_long(FFB_FEATURE_IDX, FFB_FN_RESET_ALL, NULL, 0);
}

static void ffb_upload_constant(int16_t force, uint16_t duration_ms) {
    // Quirk G923 Xbox: signo invertido en wire
    int16_t wire = -force;
    uint8_t p[14];
    memset(p, 0, sizeof(p));
    p[1] = FFB_EFFECT_CONSTANT | FFB_AUTOSTART;
    p[2] = (duration_ms >> 8) & 0xFF;
    p[3] = duration_ms & 0xFF;
    p[6] = ((uint16_t)wire >> 8) & 0xFF;
    p[7] = (uint16_t)wire & 0xFF;
    hidpp_send_long(FFB_FEATURE_IDX, FFB_FN_DOWNLOAD, p, sizeof(p));
}

static void ffb_upload_spring(int16_t coefficient, uint16_t saturation) {
    uint8_t p[18];
    memset(p, 0, sizeof(p));
    p[1] = FFB_EFFECT_SPRING | FFB_AUTOSTART;
    // left saturation (u15 BE)
    p[6] = (saturation >> 9) & 0xFF;
    p[7] = (saturation >> 1) & 0xFF;
    // left coefficient (i16 BE)
    p[8] = ((uint16_t)coefficient >> 8) & 0xFF;
    p[9] = (uint16_t)coefficient & 0xFF;
    // right coefficient
    p[14] = p[8];
    p[15] = p[9];
    // right saturation
    p[16] = p[6];
    p[17] = p[7];
    hidpp_send_vlong(FFB_FEATURE_IDX, FFB_FN_DOWNLOAD, p, sizeof(p));
}

// ════════════════════════════════════════════════════════════════════
// Shared memory (para monitor externo)
// ════════════════════════════════════════════════════════════════════

static int shm_create(void) {
    int fd = shm_open(G923_SHM_NAME, O_CREAT | O_RDWR, 0666);
    if (fd < 0) return 0;
    ftruncate(fd, sizeof(g923_telemetry_t));
    g_shm = mmap(NULL, sizeof(g923_telemetry_t),
                  PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    close(fd);
    if (g_shm == MAP_FAILED) { g_shm = NULL; return 0; }
    g_shm->magic = G923_MAGIC;
    g_shm->version = 1;
    return 1;
}

static void shm_destroy(void) {
    if (g_shm) { munmap(g_shm, sizeof(g923_telemetry_t)); g_shm = NULL; }
    shm_unlink(G923_SHM_NAME);
}

// Structs para el evento de configuración (cargo mass viene por aquí)
typedef struct {
    scs_string_t name;       // 8
    scs_u32_t    index;      // 4
    scs_u32_t    _pad;       // 4
    scs_value_t  value;      // 48
} scs_named_value_t;         // 64 bytes en x64

typedef struct {
    scs_string_t             id;         // "job", "truck", "trailer.0", etc.
    const scs_named_value_t *attributes; // array terminado en name=NULL
} scs_telemetry_configuration_t;

// ════════════════════════════════════════════════════════════════════
// Telemetry callbacks
// ════════════════════════════════════════════════════════════════════

static void on_float(scs_string_t n, scs_u32_t i,
                     const scs_value_t *v, scs_context_t ctx) {
    if (v) *(float *)ctx = v->value_float;
}
static void on_fvector(scs_string_t n, scs_u32_t i,
                       const scs_value_t *v, scs_context_t ctx) {
    if (!v) return;
    float *dst = (float *)ctx;
    dst[0] = v->value_fvector.x;
    dst[1] = v->value_fvector.y;
    dst[2] = v->value_fvector.z;
}
static void on_indexed_float(scs_string_t n, scs_u32_t i,
                             const scs_value_t *v, scs_context_t ctx) {
    if (v && i < 4) ((float *)ctx)[i] = v->value_float;
}
static void on_indexed_bool(scs_string_t n, scs_u32_t i,
                            const scs_value_t *v, scs_context_t ctx) {
    if (v && i < 4) ((uint8_t *)ctx)[i] = v->value_bool;
}

// ════════════════════════════════════════════════════════════════════
// FFB update (llamado cada N frames)
// ════════════════════════════════════════════════════════════════════

static int g_ffb_logged = 0;

static void ffb_update(void) {
    if (!g_ffb_ready) return;

    g_ffb_frame++;

    if (!g_ffb_logged) {
        char msg[128];
        snprintf(msg, sizeof(msg),
            "[g923] ffb_update llamado! speed=%.1f accel_x=%.2f paused=%d",
            g_telem.speed * 3.6f, g_telem.accel_x, g_telem.paused);
        log_msg(msg);
        g_ffb_logged = 1;
    }

    // Cleanup periódico para no agotar los 64 slots
    if ((g_ffb_frame % FFB_CLEANUP_INTERVAL) == 0) {
        ffb_reset_all();
        g_last_spring_coeff = 0;
        g_last_lateral = 0;
    }

    float speed_kmh = g_telem.speed * 3.6f;

    // Spring: autocentrado proporcional a velocidad
    float coeff = SPRING_BASE + speed_kmh * SPRING_PER_KMH;
    if (coeff > SPRING_MAX) coeff = SPRING_MAX;

    if (fabsf(coeff - g_last_spring_coeff) > SPRING_THRESHOLD) {
        ffb_upload_spring((int16_t)coeff, 0xFFFF);
        g_last_spring_coeff = coeff;
    }

    // Lateral: fuerza por aceleración en curvas
    float lat = g_telem.accel_x * LATERAL_GAIN;
    if (lat > LATERAL_MAX) lat = LATERAL_MAX;
    if (lat < -LATERAL_MAX) lat = -LATERAL_MAX;

    if (fabsf(lat - g_last_lateral) > LATERAL_THRESHOLD) {
        if (fabsf(lat) > LATERAL_DEADZONE) {
            ffb_upload_constant((int16_t)lat, 300);
        }
        g_last_lateral = lat;
    }
}

// ════════════════════════════════════════════════════════════════════
// Event callback
// ════════════════════════════════════════════════════════════════════

static int g_frame_skip = 0;

static void on_event(scs_u32_t event, const void *info, scs_context_t ctx) {
    switch (event) {
    case SCS_TELEMETRY_EVENT_frame_end:
        g_telem.frame++;
        if (g_shm) memcpy(g_shm, &g_telem, sizeof(g923_telemetry_t));
        g_frame_skip++;
        if (g_frame_skip >= FFB_UPDATE_INTERVAL) {
            g_frame_skip = 0;
            if (!g_telem.paused) ffb_update();
        }
        break;
    case SCS_TELEMETRY_EVENT_paused:
        g_telem.paused = 1;
        if (g_ffb_ready) ffb_reset_all();
        g_last_spring_coeff = 0;
        g_last_lateral = 0;
        break;
    case SCS_TELEMETRY_EVENT_started:
        g_telem.paused = 0;
        break;
    case SCS_TELEMETRY_EVENT_configuration: {
        if (!info) break;
        const scs_telemetry_configuration_t *cfg = info;
        if (!cfg->id || !cfg->attributes) break;
        // Buscar cargo.mass en configuraciones "job" o "trailer"
        for (const scs_named_value_t *attr = cfg->attributes; attr->name; attr++) {
            if (strcmp(attr->name, "cargo.mass") == 0 &&
                attr->value.type == SCS_VALUE_TYPE_float) {
                g_telem.cargo_mass = attr->value.value_float;
                char msg[64];
                snprintf(msg, sizeof(msg), "[g923] Carga detectada: %.0f kg", g_telem.cargo_mass);
                log_msg(msg);
            }
        }
        break;
    }
    }
}

// ════════════════════════════════════════════════════════════════════
// Exported API
// ════════════════════════════════════════════════════════════════════

#define REG_FLOAT(ch, field) \
    reg_ch(ch, SCS_U32_NIL, SCS_VALUE_TYPE_float, flag, on_float, &g_telem.field)

__attribute__((visibility("default")))
scs_result_t scs_telemetry_init(scs_u32_t version,
                                const scs_telemetry_init_params_t *params) {
    if (!params) return SCS_RESULT_generic_error;
    g_log = params->common.log;

    if (version < SCS_TELEMETRY_VERSION_1_01) {
        log_msg("[g923] SDK version demasiado vieja");
        return SCS_RESULT_unsupported;
    }

    memset(&g_telem, 0, sizeof(g_telem));
    g_telem.magic = G923_MAGIC;
    g_telem.version = 1;

    // Shared memory (opcional, para el monitor externo)
    if (shm_create())
        log_msg("[g923] Shared memory creada: /g923_telemetry");

    // Abrir G923 HID++ para FFB
    if (hid_open()) {
        g_ffb_ready = 1;
        ffb_reset_all();
        log_msg("[g923] FFB listo — feature_idx=11 (ForceFeedback 0x8123)");
    } else {
        log_msg("[g923] FFB no disponible (volante no encontrado). Solo telemetria.");
    }

    // Registrar eventos
    if (params->register_for_event) {
        params->register_for_event(SCS_TELEMETRY_EVENT_frame_end,      on_event, NULL);
        params->register_for_event(SCS_TELEMETRY_EVENT_paused,        on_event, NULL);
        params->register_for_event(SCS_TELEMETRY_EVENT_started,       on_event, NULL);
        params->register_for_event(SCS_TELEMETRY_EVENT_configuration, on_event, NULL);
    }

    // Registrar canales de telemetría
    if (params->register_for_channel) {
        scs_telemetry_register_for_channel_t reg_ch = params->register_for_channel;
        scs_u32_t flag = SCS_TELEMETRY_CHANNEL_FLAG_each_frame;

        REG_FLOAT("truck.speed",              speed);
        REG_FLOAT("truck.engine.rpm",         rpm);
        REG_FLOAT("truck.effective.steering",  steering);
        REG_FLOAT("truck.input.throttle",     throttle);
        REG_FLOAT("truck.input.brake",        brake);
        REG_FLOAT("truck.input.clutch",       clutch);
        reg_ch("truck.local.acceleration.linear", SCS_U32_NIL,
               SCS_VALUE_TYPE_fvector, flag, on_fvector, &g_telem.accel_x);

        // Canales indexados: registrar cada rueda individualmente
        for (scs_u32_t w = 0; w < 4; w++) {
            reg_ch("truck.wheel.suspension.deflection", w,
                   SCS_VALUE_TYPE_float, flag, on_indexed_float, g_telem.susp_deflection);
            reg_ch("truck.wheel.on_ground", w,
                   SCS_VALUE_TYPE_bool, flag, on_indexed_bool, g_telem.on_ground);
        }
    }

    log_msg("[g923] Plugin inicializado — telemetria + FFB in-process");
    return SCS_RESULT_ok;
}

__attribute__((visibility("default")))
void scs_telemetry_shutdown(void) {
    if (g_ffb_ready) {
        ffb_reset_all();
        g_ffb_ready = 0;
    }
    hid_close();
    shm_destroy();
    log_msg("[g923] Plugin cerrado");
}
