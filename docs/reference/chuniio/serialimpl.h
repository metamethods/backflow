/*
    Serial LED implementation for chuniio
    
    Credits:
    somewhatlurker, skogaby
*/

#pragma once

#include <windows.h>
#include <stdint.h>

#include "chuniio/leddata.h"

HRESULT led_serial_init(wchar_t led_com[12], DWORD baud);
void led_serial_update(struct _chuni_led_data_buf_t* data);
void led_serial_update_openithm(const uint8_t* rgb);