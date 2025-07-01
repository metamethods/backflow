#include "mai2io/mai2io.h"

#include <limits.h>
#include <process.h>

#include "mai2hook/touch.h"
#include "mai2io/config.h"
#include "util/dprintf.h"
#include "util/env.h"

static uint8_t mai2_opbtn;
static uint16_t mai2_player1_btn;
static uint16_t mai2_player2_btn;
static struct mai2_io_config mai2_io_cfg;
static bool mai2_io_coin;
mai2_io_touch_callback_t _callback;
static HANDLE mai2_io_touch_1p_thread;
static bool mai2_io_touch_1p_stop_flag;
static HANDLE mai2_io_touch_2p_thread;
static bool mai2_io_touch_2p_stop_flag;

uint16_t mai2_io_get_api_version(void) { return 0x0101; }

HRESULT mai2_io_init(void) {
    mai2_io_config_load(&mai2_io_cfg, get_config_path());

    return S_OK;
}

HRESULT mai2_io_poll(void) {
    mai2_opbtn = 0;
    mai2_player1_btn = 0;
    mai2_player2_btn = 0;

    if (GetAsyncKeyState(mai2_io_cfg.vk_test) & 0x8000) {
        mai2_opbtn |= MAI2_IO_OPBTN_TEST;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_service) & 0x8000) {
        mai2_opbtn |= MAI2_IO_OPBTN_SERVICE;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_coin) & 0x8000) {
        if (!mai2_io_coin) {
            mai2_io_coin = true;
            mai2_opbtn |= MAI2_IO_OPBTN_COIN;
        }
    } else {
        mai2_io_coin = false;
    }
    // If sinmai has enabled DebugInput, there is no need to input buttons
    // through hook amdaemon.
    if (!mai2_io_cfg.vk_btn_enable) {
        return S_OK;
    }

    // Player 1
    if (GetAsyncKeyState(mai2_io_cfg.vk_1p_btn[0])) {
        mai2_player1_btn |= MAI2_IO_GAMEBTN_1;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_1p_btn[1])) {
        mai2_player1_btn |= MAI2_IO_GAMEBTN_2;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_1p_btn[2])) {
        mai2_player1_btn |= MAI2_IO_GAMEBTN_3;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_1p_btn[3])) {
        mai2_player1_btn |= MAI2_IO_GAMEBTN_4;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_1p_btn[4])) {
        mai2_player1_btn |= MAI2_IO_GAMEBTN_5;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_1p_btn[5])) {
        mai2_player1_btn |= MAI2_IO_GAMEBTN_6;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_1p_btn[6])) {
        mai2_player1_btn |= MAI2_IO_GAMEBTN_7;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_1p_btn[7])) {
        mai2_player1_btn |= MAI2_IO_GAMEBTN_8;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_1p_btn[8])) {
        mai2_player1_btn |= MAI2_IO_GAMEBTN_SELECT;
    }

    // Player 2
    if (GetAsyncKeyState(mai2_io_cfg.vk_2p_btn[0])) {
        mai2_player2_btn |= MAI2_IO_GAMEBTN_1;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_2p_btn[1])) {
        mai2_player2_btn |= MAI2_IO_GAMEBTN_2;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_2p_btn[2])) {
        mai2_player2_btn |= MAI2_IO_GAMEBTN_3;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_2p_btn[3])) {
        mai2_player2_btn |= MAI2_IO_GAMEBTN_4;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_2p_btn[4])) {
        mai2_player2_btn |= MAI2_IO_GAMEBTN_5;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_2p_btn[5])) {
        mai2_player2_btn |= MAI2_IO_GAMEBTN_6;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_2p_btn[6])) {
        mai2_player2_btn |= MAI2_IO_GAMEBTN_7;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_2p_btn[7])) {
        mai2_player2_btn |= MAI2_IO_GAMEBTN_8;
    }

    if (GetAsyncKeyState(mai2_io_cfg.vk_2p_btn[8])) {
        mai2_player2_btn |= MAI2_IO_GAMEBTN_SELECT;
    }

    return S_OK;
}

void mai2_io_get_opbtns(uint8_t *opbtn) {
    if (opbtn != NULL) {
        *opbtn = mai2_opbtn;
    }
}

void mai2_io_get_gamebtns(uint16_t *player1, uint16_t *player2) {
    if (player1 != NULL) {
        *player1 = mai2_player1_btn;
    }

    if (player2 != NULL) {
        *player2 = mai2_player2_btn;
    }
}

HRESULT mai2_io_touch_init(mai2_io_touch_callback_t callback) {
    _callback = callback;
    return S_OK;
}

void mai2_io_touch_set_sens(uint8_t *bytes) {
#if 0
    dprintf("Mai2 touch side %c: set sensor %s sensitivity to %d\n", bytes[1], sensor_to_str(bytes[2]), bytes[4]);
#endif
    return;
}

void mai2_io_touch_update(bool player1, bool player2) {
    if (mai2_io_cfg.debug_input_1p) {
        if (player1 && mai2_io_touch_1p_thread == NULL) {
            mai2_io_touch_1p_thread = (HANDLE)_beginthreadex(
                NULL, 0, mai2_io_touch_1p_thread_proc, _callback, 0, NULL);
        } else if (!player1 && mai2_io_touch_1p_thread != NULL) {
            mai2_io_touch_1p_stop_flag = true;

            WaitForSingleObject(mai2_io_touch_1p_thread, INFINITE);
            CloseHandle(mai2_io_touch_1p_thread);
            mai2_io_touch_1p_thread = NULL;

            mai2_io_touch_1p_stop_flag = false;
        }
    }

    if (mai2_io_cfg.debug_input_2p) {
        if (player2 && mai2_io_touch_2p_thread == NULL) {
            mai2_io_touch_2p_thread = (HANDLE)_beginthreadex(
                NULL, 0, mai2_io_touch_2p_thread_proc, _callback, 0, NULL);
        } else if (!player2 && mai2_io_touch_2p_thread != NULL) {
            mai2_io_touch_2p_stop_flag = true;

            WaitForSingleObject(mai2_io_touch_2p_thread, INFINITE);
            CloseHandle(mai2_io_touch_2p_thread);
            mai2_io_touch_2p_thread = NULL;

            mai2_io_touch_2p_stop_flag = false;
        }
    }
}

static unsigned int __stdcall mai2_io_touch_1p_thread_proc(void *ctx) {
    mai2_io_touch_callback_t callback = ctx;

    while (!mai2_io_touch_1p_stop_flag) {
        uint8_t state[7] = {0, 0, 0, 0, 0, 0, 0};

        for (int i = 0; i < 34; i++) {
            if (GetAsyncKeyState(mai2_io_cfg.vk_1p_touch[i])) {
                int byteIndex = i / 5;
                int bitIndex = i % 5;
                state[byteIndex] |= (1 << bitIndex);
            }
        }
        callback(1, state);
        Sleep(1);
    }
    return 0;
}

static unsigned int __stdcall mai2_io_touch_2p_thread_proc(void *ctx) {
    mai2_io_touch_callback_t callback = ctx;

    while (!mai2_io_touch_2p_stop_flag) {
        uint8_t state[7] = {0, 0, 0, 0, 0, 0, 0};

        for (int i = 0; i < 34; i++) {
            if (GetAsyncKeyState(mai2_io_cfg.vk_2p_touch[i])) {
                int byteIndex = i / 5;
                int bitIndex = i % 5;
                state[byteIndex] |= (1 << bitIndex);
            }
        }
        callback(2, state);
        Sleep(1);
    }
    return 0;
}

HRESULT mai2_io_led_init(void) { return S_OK; }

void mai2_io_led_set_fet_output(uint8_t board, const uint8_t *rgb) {
#if 0
    uint8_t player = board + 1;
    dprintf("MAI2 LED %dP: BodyLed brightness: %d%%\n", player,
            (rgb[0] * 100) / 255);
    dprintf("MAI2 LED %dP: ExtLed brightness: %d%%\n", player,
            (rgb[1] * 100) / 255);
    dprintf("MAI2 LED %dP: SideLed brightness: %d%%\n", player,
            (rgb[2] * 100) / 255);
#endif
    return;
}

void mai2_io_led_dc_update(uint8_t board, const uint8_t *rgb) {
#if 0
    uint8_t player = board + 1;
    for (int i = 0; i < 10; i++) {
        dprintf("Mai2 LED %dP: LED %d: %02X %02X %02X Speed: %02X\n", player
                i, rgb[i * 4], rgb[i * 4 + 1], rgb[i * 4 + 2], rgb[i * 4 + 3]);
    }
#endif
    return;
}

void mai2_io_led_gs_update(uint8_t board, const uint8_t *rgb) {
#if 0
    uint8_t player = board + 1;
    for (int i = 0; i < 8; i++) {
        dprintf("Mai2 LED %dP: LED %d: %02X %02X %02X Speed: %02X\n", player, i,
                rgb[i * 4], rgb[i * 4 + 1], rgb[i * 4 + 2], rgb[i * 4 + 3]);
    }
#endif
    return;
}
