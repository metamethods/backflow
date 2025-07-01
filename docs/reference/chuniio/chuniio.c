#include <windows.h>

#include <process.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "chuniio/chuniio.h"
#include "chuniio/config.h"
#include "chuniio/ledoutput.h"

#include "util/dprintf.h"
#include "util/env.h"

static unsigned int __stdcall chuni_io_slider_thread_proc(void *ctx);

static bool chuni_io_coin;
static uint16_t chuni_io_coins;
static uint8_t chuni_io_hand_pos;
static HANDLE chuni_io_slider_thread;
static bool chuni_io_slider_stop_flag;
static struct chuni_io_config chuni_io_cfg;

uint16_t chuni_io_get_api_version(void)
{
    return 0x0102;
}

HRESULT chuni_io_jvs_init(void)
{
    chuni_io_config_load(&chuni_io_cfg, get_config_path());
    
    led_init_mutex = CreateMutex(
        NULL,              // default security attributes
        FALSE,             // initially not owned
        NULL);             // unnamed mutex
    
    if (led_init_mutex == NULL)
    {
        return E_FAIL;
    }
    
    return S_OK;
}

void chuni_io_jvs_read_coin_counter(uint16_t *out)
{
    if (out == NULL) {
        return;
    }

    if (GetAsyncKeyState(chuni_io_cfg.vk_coin) & 0x8000) {
        if (!chuni_io_coin) {
            chuni_io_coin = true;
            chuni_io_coins++;
        }
    } else {
        chuni_io_coin = false;
    }

    *out = chuni_io_coins;
}

void chuni_io_jvs_poll(uint8_t *opbtn, uint8_t *beams)
{
    size_t i;

    if (GetAsyncKeyState(chuni_io_cfg.vk_test) & 0x8000) {
        *opbtn |= CHUNI_IO_OPBTN_TEST;
    }

    if (GetAsyncKeyState(chuni_io_cfg.vk_service) & 0x8000) {
        *opbtn |= CHUNI_IO_OPBTN_SERVICE;
    }

    if (chuni_io_cfg.vk_ir_emu) {
        // Use emulated AIR
        if (GetAsyncKeyState(chuni_io_cfg.vk_ir_emu)) {
            if (chuni_io_hand_pos < 6) {
                chuni_io_hand_pos++;
            }
        } else {
            if (chuni_io_hand_pos > 0) {
                chuni_io_hand_pos--;
            }
        }

        for (i = 0 ; i < 6 ; i++) {
            if (chuni_io_hand_pos > i) {
                *beams |= (1 << i);
            }
        }
    } else {
        // Use actual AIR
        for (i = 0; i < 6; i++) {
            if (GetAsyncKeyState(chuni_io_cfg.vk_ir[i]) & 0x8000) {
                *beams |= (1 << i);
            } else {
                *beams &= ~(1 << i);
            }
        }
    }
}

HRESULT chuni_io_slider_init(void)
{
    return led_output_init(&chuni_io_cfg); // because of slider LEDs
}

void chuni_io_slider_start(chuni_io_slider_callback_t callback)
{
    BOOL status;

    if (chuni_io_slider_thread != NULL) {
        return;
    }

    chuni_io_slider_thread = (HANDLE) _beginthreadex(
            NULL,
            0,
            chuni_io_slider_thread_proc,
            callback,
            0,
            NULL);
}

void chuni_io_slider_stop(void)
{
    if (chuni_io_slider_thread == NULL) {
        return;
    }

    chuni_io_slider_stop_flag = true;

    WaitForSingleObject(chuni_io_slider_thread, INFINITE);
    CloseHandle(chuni_io_slider_thread);
    chuni_io_slider_thread = NULL;
    chuni_io_slider_stop_flag = false;
}

void chuni_io_slider_set_leds(const uint8_t *rgb)
{
    led_output_update(2, rgb);
}

static unsigned int __stdcall chuni_io_slider_thread_proc(void *ctx)
{
    chuni_io_slider_callback_t callback;
    uint8_t pressure[32];
    size_t i;

    callback = ctx;

    while (!chuni_io_slider_stop_flag) {
        for (i = 0 ; i < _countof(pressure) ; i++) {
            if (GetAsyncKeyState(chuni_io_cfg.vk_cell[i]) & 0x8000) {
                pressure[i] = 128;
            } else {
                pressure[i] = 0;
            }
        }

        callback(pressure);
        Sleep(1);
    }

    return 0;
}

HRESULT chuni_io_led_init(void)
{
    return led_output_init(&chuni_io_cfg);
}

void chuni_io_led_set_colors(uint8_t board, uint8_t *rgb)
{
#if 0
    if (board == 0) {
        dprintf("CHUNI LED: Left Air 1: red: %d, green: %d, blue: %d\n", rgb[0x96], rgb[0x97], rgb[0x98]);
        dprintf("CHUNI LED: Left Air 2: red: %d, green: %d, blue: %d\n", rgb[0x99], rgb[0x9A], rgb[0x9B]);
        dprintf("CHUNI LED: Left Air 3: red: %d, green: %d, blue: %d\n", rgb[0x9C], rgb[0x9D], rgb[0x9E]);
	}
	else if (board == 1)
	{
		dprintf("CHUNI LED: Right Air 1: red: %d, green: %d, blue: %d\n", rgb[0xB4], rgb[0xB5], rgb[0xB6]);
        dprintf("CHUNI LED: Right Air 2: red: %d, green: %d, blue: %d\n", rgb[0xB7], rgb[0xB8], rgb[0xB9]);
        dprintf("CHUNI LED: Right Air 3: red: %d, green: %d, blue: %d\n", rgb[0xBA], rgb[0xBB], rgb[0xBC]);
    }
#endif

    led_output_update(board, rgb);
}
