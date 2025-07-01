#include <windows.h>

#include <process.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>

#include "chuniio/chu2to3.h"
#include "util/dprintf.h"

// Check windows
#if _WIN32 || _WIN64
    #if _WIN64
        #define ENV64BIT
    #else
        #define ENV32BIT
    #endif
#endif

// Check GCC
#if __GNUC__
    #if __x86_64__ || __ppc64__
        #define ENV64BIT
    #else
        #define ENV32BIT
    #endif
#endif

/* chuniio.dll dynamic loading */
HMODULE hinstLib;
typedef uint16_t (*chuni_io_get_api_version_t)(void);
typedef HRESULT (*chuni_io_jvs_init_t)(void);
typedef void (*chuni_io_jvs_poll_t)(uint8_t*, uint8_t*);
typedef void (*chuni_io_jvs_read_coin_counter_t)(uint16_t *);
typedef HRESULT (*chuni_io_slider_init_t)(void);
typedef void (*chuni_io_slider_set_leds_t)(const uint8_t *);
typedef void (*chuni_io_slider_start_t)(chuni_io_slider_callback_t);
typedef void (*chuni_io_slider_stop_t)(void);
typedef HRESULT (*chuni_io_led_init_t)(void);
typedef void (*chuni_io_led_set_colors_t)(uint8_t, uint8_t *);

chuni_io_get_api_version_t _chuni_io_get_api_version;
chuni_io_jvs_init_t _chuni_io_jvs_init;
chuni_io_jvs_poll_t _chuni_io_jvs_poll;
chuni_io_jvs_read_coin_counter_t _chuni_io_jvs_read_coin_counter;
chuni_io_slider_init_t _chuni_io_slider_init;
chuni_io_slider_set_leds_t _chuni_io_slider_set_leds;
chuni_io_slider_start_t _chuni_io_slider_start;
chuni_io_slider_stop_t _chuni_io_slider_stop;
chuni_io_led_init_t _chuni_io_led_init;
chuni_io_led_set_colors_t _chuni_io_led_set_colors;

/* SHMEM Handling */
#define BUF_SIZE 1024
#define SHMEM_WRITE(buf, size) CopyMemory((PVOID)g_pBuf, buf, size)
#define SHMEM_READ(buf, size) CopyMemory(buf,(PVOID)g_pBuf, size)
TCHAR g_shmem_name[]=TEXT("Local\\Chu2to3Shmem");
HANDLE g_hMapFile;
LPVOID g_pBuf;

#pragma pack(1)
typedef struct shared_data_s {
    uint16_t coin_counter;
    uint8_t  opbtn;
    uint8_t  beams;
    uint16_t version;
} shared_data_t;

shared_data_t g_shared_data;

bool shmem_create()
{
   g_hMapFile = CreateFileMapping(
                                  INVALID_HANDLE_VALUE,    // use paging file
                                  NULL,                    // default security
                                  PAGE_READWRITE,          // read/write access
                                  0,                       // maximum object size (high-order DWORD)
                                  BUF_SIZE,                // maximum object size (low-order DWORD)
                                  g_shmem_name);           // name of mapping object

   if (g_hMapFile == NULL)
   {
      dprintf("shmem_create : Could not create file mapping object (%ld).\n",
             GetLastError());
      return 0;
   }
   g_pBuf = MapViewOfFile(g_hMapFile,          // handle to map object
                          FILE_MAP_ALL_ACCESS, // read/write permission
                          0,
                          0,
                          BUF_SIZE);

   if (g_pBuf == NULL)
   {
      dprintf("shmem_create : Could not map view of file (%ld).\n",
             GetLastError());

       CloseHandle(g_hMapFile);

      return 0;
   }

    return 1;
}

bool shmem_load()
{
    g_hMapFile = OpenFileMapping(
                                 FILE_MAP_ALL_ACCESS,   // read/write access
                                 FALSE,                 // do not inherit the name
                                 g_shmem_name);         // name of mapping object

    if (g_hMapFile == NULL)
    {
        dprintf("shmem_load : Could not open file mapping object (%ld).\n", GetLastError());
        return 0;
   }

    g_pBuf = MapViewOfFile(g_hMapFile,           // handle to map object
                           FILE_MAP_ALL_ACCESS,  // read/write permission
                           0,
                           0,
                           BUF_SIZE);

    if (g_pBuf == NULL)
    {
        dprintf("shmem_load : Could not map view of file (%ld).\n", GetLastError());
        CloseHandle(g_hMapFile);
        return 0;
    }

    dprintf("shmem_load : shmem loaded succesfully.\n");
    return 1;
}

void shmem_free()
{
   UnmapViewOfFile(g_pBuf);
   CloseHandle(g_hMapFile);
}

/* jvs polling thread (to forward info to x64 dll) */
static HANDLE jvs_poll_thread;
static bool jvs_poll_stop_flag;

static unsigned int __stdcall jvs_poll_thread_proc(void *ctx)
{
    while (1) {
        _chuni_io_jvs_read_coin_counter(&g_shared_data.coin_counter);
        g_shared_data.opbtn = 0;
        g_shared_data.beams = 0;
        _chuni_io_jvs_poll(&g_shared_data.opbtn, &g_shared_data.beams);
        SHMEM_WRITE(&g_shared_data, sizeof(shared_data_t));
        Sleep(1);
    }

    return 0;
}

uint16_t chu2to3_load_dll(const wchar_t *dllname)
{
    #if defined(ENV64BIT)
    /* x64 must just open the shmem and do nothing else */
    int errcount = 0;
    while (!shmem_load())
    {
        if (errcount >= 10)
            return -1;
        Sleep(5000);
        errcount++;
    }
    Sleep(1000);
    return S_OK;
    #endif

    /* this is the first function called so let's setup the chuniio forwarding */
    hinstLib = LoadLibraryW(dllname);
    if (hinstLib == NULL) {
        dprintf("ERROR: unable to load %S (error %ld)\n",dllname, GetLastError());
        return -1;
    }

    _chuni_io_get_api_version = (chuni_io_get_api_version_t)GetProcAddress(hinstLib, "chuni_io_get_api_version");
    _chuni_io_jvs_init = (chuni_io_jvs_init_t)GetProcAddress(hinstLib, "chuni_io_jvs_init");
    _chuni_io_jvs_poll = (chuni_io_jvs_poll_t)GetProcAddress(hinstLib, "chuni_io_jvs_poll");
    _chuni_io_jvs_read_coin_counter = (chuni_io_jvs_read_coin_counter_t)GetProcAddress(hinstLib, "chuni_io_jvs_read_coin_counter");
    _chuni_io_slider_init = (chuni_io_slider_init_t)GetProcAddress(hinstLib, "chuni_io_slider_init");
    _chuni_io_slider_set_leds = (chuni_io_slider_set_leds_t)GetProcAddress(hinstLib, "chuni_io_slider_set_leds");
    _chuni_io_slider_start = (chuni_io_slider_start_t)GetProcAddress(hinstLib, "chuni_io_slider_start");
    _chuni_io_slider_stop = (chuni_io_slider_stop_t)GetProcAddress(hinstLib, "chuni_io_slider_stop");
    _chuni_io_led_init = (chuni_io_led_init_t)GetProcAddress(hinstLib, "chuni_io_led_init");
    _chuni_io_led_set_colors = (chuni_io_led_set_colors_t)GetProcAddress(hinstLib, "chuni_io_led_set_colors");

    /* x86 has to create the shmem */
    if (!shmem_create())
    {
        return -1;
    }

    return 0;
}

/* chuniio exports */
uint16_t chu2to3_io_get_api_version(void)
{
    #if defined(ENV64BIT)
        /* This might be called too soon so let's make sure x86 has time to write to the shmem */
        SHMEM_READ(&g_shared_data, sizeof(shared_data_t));
        int errcount = 0;
        while (g_shared_data.version == 0)
        {
            if (errcount >= 3)
            {
                dprintf("CHU2TO3 X64: Couldn't retrieve api version from shmem, assuming 0x0100\n");
                return 0x0100;
            }
            Sleep(5000);
            errcount++;
            SHMEM_READ(&g_shared_data, sizeof(shared_data_t));
        }
        dprintf("CHU2TO3 X64: api version is %04X\n", g_shared_data.version);
        return g_shared_data.version;
    #endif

    if ( _chuni_io_get_api_version == NULL )
    {
        g_shared_data.version = 0x0100;
    }
    else
    {
        g_shared_data.version = _chuni_io_get_api_version();
    }
    dprintf("CHU2TO3: api version is %04X\n", g_shared_data.version);

    SHMEM_WRITE(&g_shared_data, sizeof(shared_data_t));

    return g_shared_data.version;
}

HRESULT chu2to3_io_jvs_init(void)
{
    #if defined(ENV64BIT)
    /* x86 only */
    return S_OK;
    #endif

    _chuni_io_jvs_init();

    /* start jvs poll thread now that jvs_init is done */
    if (jvs_poll_thread != NULL) {
        return S_OK;
    }

    jvs_poll_thread = (HANDLE) _beginthreadex(NULL,
                                              0,
                                              jvs_poll_thread_proc,
                                              NULL,
                                              0,
                                              NULL);
    return S_OK;
}

void chu2to3_io_jvs_read_coin_counter(uint16_t *out)
{
    #if defined(ENV32BIT)
    /* x86 can perform the call and update shmem (although this call never happens) */
    _chuni_io_jvs_read_coin_counter(&g_shared_data.coin_counter);
    SHMEM_WRITE(&g_shared_data, sizeof(shared_data_t));
    return;
    #endif

    /* x64 must read value from shmem and update arg */
    SHMEM_READ(&g_shared_data, sizeof(shared_data_t));
    if (out == NULL) {
        return;
    }
    *out = g_shared_data.coin_counter;
}

void chu2to3_io_jvs_poll(uint8_t *opbtn, uint8_t *beams)
{
    #if defined(ENV32BIT)
    /* x86 can perform the call and update shmem (although this call never happens) */
    _chuni_io_jvs_poll(&g_shared_data.opbtn, &g_shared_data.beams);
    SHMEM_WRITE(&g_shared_data, sizeof(shared_data_t));
    return;
    #endif

    /* x64 must read value from shmem and update args */
    SHMEM_READ(&g_shared_data, sizeof(shared_data_t));
    *opbtn = g_shared_data.opbtn;
    *beams = g_shared_data.beams;
}

HRESULT chu2to3_io_slider_init(void)
{
    #if defined(ENV64BIT)
    /* x86 only */
    return S_OK;
    #endif

    return _chuni_io_slider_init();
}

void chu2to3_io_slider_start(chuni_io_slider_callback_t callback)
{
    #if defined(ENV64BIT)
    /* x86 only */
    return;
    #endif

    _chuni_io_slider_start(callback);
}

void chu2to3_io_slider_stop(void)
{
    #if defined(ENV64BIT)
    /* x86 only */
    return;
    #endif

    _chuni_io_slider_stop();
}

void chu2to3_io_slider_set_leds(const uint8_t *rgb)
{
    #if defined(ENV64BIT)
    /* x86 only */
    return;
    #endif

    _chuni_io_slider_set_leds(rgb);
}

HRESULT chu2to3_io_led_init(void)
{
    #if defined(ENV64BIT)
    /* x86 only */
    return S_OK;
    #endif

    if (_chuni_io_led_init != NULL)
        return _chuni_io_led_init();
    return S_OK;
}

void chu2to3_io_led_set_colors(uint8_t board, uint8_t *rgb)
{
    #if defined(ENV64BIT)
    /* x86 only */
    return;
    #endif

    if (_chuni_io_led_set_colors != NULL)
    {
        _chuni_io_led_set_colors(board, rgb);
    }
}
