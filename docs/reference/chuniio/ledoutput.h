/*
    LED output functions
    
    Credits:
    somewhatlurker, skogaby
*/

#pragma once

#include <windows.h>

#include <stdbool.h>
#include <stdint.h>

#include "chuniio/config.h"

extern HANDLE led_init_mutex;
HRESULT led_output_init(struct chuni_io_config* const cfg);
void led_output_update(uint8_t board, const uint8_t* rgb);
